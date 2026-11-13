//! Walks the CEL AST once and turns it into typed closures. The README lists
//! the supported subset; anything else (macros, maps, indexing) is a compile
//! error.

use {
    crate::{
        EvalError,
        fee,
        node::{Node, Path, Set, Text, Val, map2},
        number::{Kind, Num, Number, Pair, arithmetic as number_arithmetic},
        order::{FieldKind, Getter, Input, ORDER, Object},
    },
    alloy_primitives::{Address, I256, U256},
    cel::common::ast::{CallExpr, Expr, IdedExpr, LiteralValue, SelectExpr, SourceInfo},
    chrono::{DateTime, TimeDelta, Utc},
    rust_decimal::{Decimal, RoundingStrategy, prelude::FromPrimitive},
    std::{borrow::Cow, cmp::Ordering, collections::HashMap, sync::Arc},
};

/// The names a rule can use besides `order` and `now`.
pub(crate) struct Scope<'s> {
    pub(crate) lists: &'s HashMap<String, Arc<Set>>,
    /// Where each literal is in the source, to read it back exactly.
    pub(crate) source: &'s SourceInfo,
}

type Result<T> = std::result::Result<T, String>;

pub(crate) fn compile(expr: &IdedExpr, scope: &Scope<'_>) -> Result<Node> {
    match &expr.expr {
        Expr::Literal(literal) => literal_node(expr.id, literal, scope),
        Expr::Ident(name) => ident(name, scope),
        Expr::Select(select) => select_node(select, scope),
        Expr::Call(call) => call_node(call, scope),
        Expr::List(list) => list_node(&list.elements, scope),
        Expr::Comprehension(_) => {
            Err("macros (`exists`, `all`, `map`, ...) are not supported".into())
        }
        Expr::Map(_) | Expr::Struct(_) => Err("map and struct literals are not supported".into()),
        Expr::Unspecified => Err("empty expression".into()),
    }
}

/// `2` is an int, `2u` a uint, `0.3` a decimal.
fn literal_node(id: u64, literal: &LiteralValue, scope: &Scope<'_>) -> Result<Node> {
    Ok(match literal {
        LiteralValue::Boolean(value) => Node::Bool(Val::Const(*value.inner())),
        LiteralValue::Int(value) => Node::Number(Num::Int(Val::Const(
            I256::try_from(*value.inner()).expect("an i64 fits 256 bits"),
        ))),
        LiteralValue::UInt(value) => {
            Node::Number(Num::UInt(Val::Const(U256::from(*value.inner()))))
        }
        LiteralValue::Double(value) => {
            // The parser rounded the literal to a double; read the source
            // text instead. Its span leaves out a leading minus.
            let value = *value.inner();
            let text = scope
                .source
                .offset_for(id)
                .and_then(|(start, stop)| scope.source.source.get(start as usize..=stop as usize))
                .map_or_else(|| value.abs().to_string(), str::to_owned);
            Node::Number(decimal_literal(&text, value.is_sign_negative())?)
        }
        LiteralValue::String(value) => Node::Text(Text::Const(value.inner().to_owned())),
        LiteralValue::Null | LiteralValue::Bytes(_) => {
            return Err("null and bytes literals are not supported".into());
        }
    })
}

/// A decimal literal exactly as written (`2.5`, `1e24`, `1.0e-2`). One too
/// large for a `Decimal` (`1e30`) is kept as an integer if it is whole.
fn decimal_literal(text: &str, negative: bool) -> Result<Num> {
    let bad = || format!("`{text}` is not a number that fits a decimal or 256 bits");
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().map_err(|_| bad())?),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = U256::from_str_radix(&format!("{whole}{fraction}"), 10).map_err(|_| bad())?;
    // value = digits / 10^scale
    let scale = i64::try_from(fraction.len()).map_err(|_| bad())? - exponent;
    let integer = (scale <= 0)
        .then(|| {
            let shift = u32::try_from(-scale).ok()?;
            digits.checked_mul(U256::from(10).checked_pow(U256::from(shift))?)
        })
        .flatten();
    let decimal = match integer {
        Some(integer) => u128::try_from(integer).ok().and_then(Decimal::from_u128),
        None => u128::try_from(digits)
            .ok()
            .and_then(|digits| i128::try_from(digits).ok())
            .zip(u32::try_from(scale).ok())
            .and_then(|(digits, scale)| Decimal::try_from_i128_with_scale(digits, scale).ok()),
    };
    match (decimal, integer) {
        (Some(decimal), _) => Ok(Num::Decimal(Val::Const(if negative {
            -decimal
        } else {
            decimal
        }))),
        (None, Some(integer)) if negative => I256::try_from(integer)
            .map(|integer| Num::Int(Val::Const(-integer)))
            .map_err(|_| bad()),
        (None, Some(integer)) => Ok(Num::UInt(Val::Const(integer))),
        (None, None) => Err(bad()),
    }
}

fn ident(name: &str, scope: &Scope<'_>) -> Result<Node> {
    match name {
        "order" => Ok(Node::Object(&ORDER)),
        "now" => Ok(Node::Timestamp(Val::Dyn(Box::new(|input| Ok(input.now))))),
        _ => scope
            .lists
            .get(name)
            .map(|set| Node::List(set.clone()))
            .ok_or_else(|| format!("unknown name `{name}`")),
    }
}

/// `a.b` and `has(a.b)`.
fn select_node(select: &SelectExpr, scope: &Scope<'_>) -> Result<Node> {
    let operand = compile(&select.operand, scope)?;
    let field = &select.field;
    match operand {
        Node::Object(object) => {
            let path = field_path(object, field)?;
            if select.test {
                return Ok(Node::Bool(present(path)));
            }
            Ok(match path {
                Path::Object(object) => Node::Object(object),
                Path::Field(field) => {
                    let FieldKind::Value(getter) = field.kind else {
                        unreachable!("field_path returns objects as Path::Object")
                    };
                    read(getter, field.name)
                }
            })
        }
        Node::Optional(Path::Object(object)) if !select.test => {
            Ok(Node::Optional(field_path(object, field)?))
        }
        other => Err(format!("a {} has no field `{field}`", other.type_name())),
    }
}

fn field_path(object: &'static Object, name: &str) -> Result<Path> {
    let field = object
        .field(name)
        .ok_or_else(|| format!("`{}` has no field `{name}`", object.name))?;
    Ok(match field.kind {
        FieldKind::Object(object) => Path::Object(object),
        FieldKind::Value(_) => Path::Field(field),
    })
}

fn present(path: Path) -> Val<bool> {
    match path {
        Path::Object(object) => {
            let present = object.present;
            Val::Dyn(Box::new(move |input| Ok(present(input))))
        }
        Path::Field(field) => {
            let FieldKind::Value(getter) = field.kind else {
                unreachable!()
            };
            Val::Dyn(Box::new(move |input| Ok(getter.present(input))))
        }
    }
}

/// A plain read: an absent field is an error, so the rule is skipped.
fn read(getter: Getter, name: &'static str) -> Node {
    let missing = move || EvalError::Missing(name);
    match getter {
        Getter::Address(get) => {
            Node::Address(Val::Dyn(Box::new(move |i| get(i).ok_or_else(missing))))
        }
        Getter::UInt(get) => Node::Number(Num::UInt(Val::Dyn(Box::new(move |i| {
            get(i).ok_or_else(missing)
        })))),
        Getter::Int(get) => Node::Number(Num::Int(Val::Dyn(Box::new(move |i| {
            get(i).ok_or_else(missing)
        })))),
        Getter::Bool(get) => Node::Bool(Val::Dyn(Box::new(move |i| get(i).ok_or_else(missing)))),
        Getter::Timestamp(get) => {
            Node::Timestamp(Val::Dyn(Box::new(move |i| get(i).ok_or_else(missing))))
        }
        Getter::Text(get) => Node::Text(Text::Dyn(Box::new(move |i| {
            get(i).map(Cow::Borrowed).ok_or_else(missing)
        }))),
    }
}

/// `optional.orValue(default)`: the field, or `default` when it is absent.
fn or_value(path: Path, default: Node) -> Result<Node> {
    let Path::Field(field) = path else {
        return Err("`orValue` on an object is not supported".into());
    };
    let FieldKind::Value(getter) = field.kind else {
        unreachable!()
    };
    fn with_default<T: Clone + Send + Sync + 'static>(
        get: fn(&Input<'_>) -> Option<T>,
        default: Val<T>,
    ) -> Val<T> {
        match default {
            Val::Const(default) => Val::Dyn(Box::new(move |i| {
                Ok(get(i).unwrap_or_else(|| default.clone()))
            })),
            Val::Dyn(default) => Val::Dyn(Box::new(move |i| match get(i) {
                Some(value) => Ok(value),
                None => default(i),
            })),
        }
    }
    Ok(match (getter, default) {
        (Getter::Address(get), Node::Address(default)) => Node::Address(with_default(get, default)),
        (Getter::UInt(get), Node::Number(default)) => match default.into_kind(Kind::UInt)? {
            Num::UInt(default) => Node::Number(Num::UInt(with_default(get, default))),
            _ => unreachable!(),
        },
        (Getter::Int(get), Node::Number(default)) => match default.into_kind(Kind::Int)? {
            Num::Int(default) => Node::Number(Num::Int(with_default(get, default))),
            _ => unreachable!(),
        },
        (Getter::Bool(get), Node::Bool(default)) => Node::Bool(with_default(get, default)),
        (Getter::Timestamp(get), Node::Timestamp(default)) => {
            Node::Timestamp(with_default(get, default))
        }
        (Getter::Text(get), Node::Text(default)) => Node::Text(match default {
            Text::Const(default) => Text::Dyn(Box::new(move |i| {
                Ok(get(i).map_or_else(|| Cow::Owned(default.clone()), Cow::Borrowed))
            })),
            Text::Dyn(default) => Text::Dyn(Box::new(move |i| match get(i) {
                Some(value) => Ok(Cow::Borrowed(value)),
                None => default(i),
            })),
        }),
        (_, default) => {
            return Err(format!(
                "`{}.orValue` default is a {}, not the field's type",
                field.name,
                default.type_name()
            ));
        }
    })
}

fn call_node(call: &CallExpr, scope: &Scope<'_>) -> Result<Node> {
    let qualified;
    let name = match call.target.as_deref() {
        // `math.floor(x)` parses as a call on the name `math`.
        Some(IdedExpr {
            expr: Expr::Ident(namespace),
            ..
        }) if namespace == "math" => {
            qualified = format!("math.{}", call.func_name);
            qualified.as_str()
        }
        Some(target) => {
            return member_call(&call.func_name, compile(target, scope)?, &call.args, scope);
        }
        None => call.func_name.as_str(),
    };
    let args = &call.args;
    let arg = |index: usize| compile(&args[index], scope);
    let arity = |n: usize| {
        if args.len() == n {
            Ok(())
        } else {
            Err(format!("`{name}` takes {n} arguments, not {}", args.len()))
        }
    };
    match name {
        "_?._" => {
            arity(2)?;
            let Node::Text(Text::Const(field)) = arg(1)? else {
                unreachable!("the parser makes the field of `.?` a string literal")
            };
            match arg(0)? {
                Node::Object(object) | Node::Optional(Path::Object(object)) => {
                    Ok(Node::Optional(field_path(object, &field)?))
                }
                other => Err(format!("a {} has no field `{field}`", other.type_name())),
            }
        }
        "_&&_" => {
            arity(2)?;
            Ok(Node::Bool(and(arg(0)?.into_bool()?, arg(1)?.into_bool()?)))
        }
        "_||_" => {
            arity(2)?;
            Ok(Node::Bool(or(arg(0)?.into_bool()?, arg(1)?.into_bool()?)))
        }
        "!_" => {
            arity(1)?;
            Ok(Node::Bool(arg(0)?.into_bool()?.map(|b| Ok(!b))?))
        }
        "_?_:_" => {
            arity(3)?;
            conditional(arg(0)?.into_bool()?, arg(1)?, arg(2)?)
        }
        "_==_" | "_!=_" => {
            arity(2)?;
            let equal = equals(arg(0)?, arg(1)?)?;
            Ok(Node::Bool(if name == "_==_" {
                equal
            } else {
                equal.map(|b| Ok(!b))?
            }))
        }
        "_<_" | "_<=_" | "_>_" | "_>=_" => {
            arity(2)?;
            let test: fn(Ordering) -> bool = match name {
                "_<_" => Ordering::is_lt,
                "_<=_" => Ordering::is_le,
                "_>_" => Ordering::is_gt,
                _ => Ordering::is_ge,
            };
            compare(arg(0)?, arg(1)?, test).map(Node::Bool)
        }
        "@in" => {
            arity(2)?;
            contains(arg(0)?, arg(1)?).map(Node::Bool)
        }
        "_+_" | "_-_" | "_*_" | "_/_" | "_%_" => {
            arity(2)?;
            arithmetic(name, arg(0)?, arg(1)?)
        }
        "-_" => {
            arity(1)?;
            match arg(0)? {
                Node::Number(Num::Decimal(d)) => Ok(Node::Number(Num::Decimal(d.map(|d| Ok(-d))?))),
                Node::Number(n) => {
                    let int = n.into_number()?.map(Number::to_int)?;
                    Ok(Node::Number(Num::Int(
                        int.map(|i| i.checked_neg().ok_or(EvalError::Arithmetic))?,
                    )))
                }
                Node::Duration(d) => Ok(Node::Duration(d.map(|d| Ok(-d))?)),
                other => Err(format!("cannot negate a {}", other.type_name())),
            }
        }
        "timestamp" => {
            arity(1)?;
            timestamp(arg(0)?)
        }
        "duration" => {
            arity(1)?;
            let Node::Text(Text::Const(text)) = arg(0)? else {
                return Err("`duration` takes a string literal".into());
            };
            Ok(Node::Duration(Val::Const(parse_duration(&text)?)))
        }
        "int" | "uint" => {
            arity(1)?;
            match arg(0)? {
                Node::Number(n) if name == "int" => Ok(Node::Number(Num::Int(
                    n.into_number()?.map(Number::to_int)?,
                ))),
                Node::Number(n) => Ok(Node::Number(Num::UInt(
                    n.into_number()?.map(Number::to_uint)?,
                ))),
                Node::Timestamp(t) if name == "int" => {
                    let seconds =
                        t.map(
                            |t| Ok(I256::try_from(t.timestamp()).expect("an i64 fits 256 bits")),
                        )?;
                    Ok(Node::Number(Num::Int(seconds)))
                }
                other => Err(format!(
                    "`{name}` of a {} is not supported",
                    other.type_name()
                )),
            }
        }
        "double" | "decimal" => {
            arity(1)?;
            let n = arg(0)?.into_number()?;
            Ok(Node::Number(Num::Decimal(
                n.into_number()?.map(Number::to_decimal)?,
            )))
        }
        "math.@max" | "math.@min" => {
            let mut numbers = args
                .iter()
                .map(|arg| compile(arg, scope)?.into_number())
                .collect::<Result<Vec<_>>>()?
                .into_iter();
            let first = numbers.next().ok_or("`math.greatest` needs an argument")?;
            let max = name == "math.@max";
            numbers
                .try_fold(first, |acc, next| extreme(acc, next, max))
                .map(Node::Number)
        }
        "math.ceil" | "math.floor" | "math.round" | "math.trunc" | "math.abs" => {
            arity(1)?;
            Ok(Node::Number(match arg(0)?.into_number()? {
                Num::Decimal(d) => {
                    let f: fn(Decimal) -> Decimal = match name {
                        "math.ceil" => |d| d.ceil(),
                        "math.floor" => |d| d.floor(),
                        "math.round" => {
                            |d| d.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                        }
                        "math.trunc" => |d| d.trunc(),
                        _ => |d| d.abs(),
                    };
                    Num::Decimal(d.map(move |d| Ok(f(d)))?)
                }
                Num::Int(i) if name == "math.abs" => {
                    Num::Int(i.map(|i| i.checked_abs().ok_or(EvalError::Arithmetic))?)
                }
                // Integers are whole already, and a uint is never negative.
                integer => integer,
            }))
        }
        "no_fee" => {
            arity(0)?;
            Ok(Node::Fees(Val::Const(Vec::new())))
        }
        _ => {
            let numbers = args
                .iter()
                .map(|arg| compile(arg, scope)?.into_number())
                .collect::<Result<Vec<_>>>();
            match fee::call(name, numbers?)? {
                Some(fee) => Ok(Node::Fee(fee)),
                None => Err(format!("unknown function `{name}`")),
            }
        }
    }
}

fn member_call(name: &str, target: Node, args: &[IdedExpr], scope: &Scope<'_>) -> Result<Node> {
    let arity = |n: usize| {
        if args.len() == n {
            Ok(())
        } else {
            Err(format!("`{name}` takes {n} arguments, not {}", args.len()))
        }
    };
    match (name, target) {
        ("orValue", Node::Optional(path)) => {
            arity(1)?;
            or_value(path, compile(&args[0], scope)?)
        }
        ("hasValue", Node::Optional(path)) => {
            arity(0)?;
            Ok(Node::Bool(present(path)))
        }
        ("lowerAscii" | "upperAscii", target) => {
            arity(0)?;
            let f: fn(Cow<'_, str>) -> Cow<'_, str> = if name == "lowerAscii" {
                |t| {
                    if t.bytes().any(|b| b.is_ascii_uppercase()) {
                        Cow::Owned(t.to_ascii_lowercase())
                    } else {
                        t
                    }
                }
            } else {
                |t| {
                    if t.bytes().any(|b| b.is_ascii_lowercase()) {
                        Cow::Owned(t.to_ascii_uppercase())
                    } else {
                        t
                    }
                }
            };
            Ok(Node::Text(target.into_text()?.map(f)))
        }
        ("startsWith" | "endsWith" | "contains", target) => {
            arity(1)?;
            let test: fn(&str, &str) -> bool = match name {
                "startsWith" => |a, b| a.starts_with(b),
                "endsWith" => |a, b| a.ends_with(b),
                _ => |a, b| a.contains(b),
            };
            let other = compile(&args[0], scope)?.into_text()?;
            Ok(Node::Bool(target.into_text()?.test(other, test)))
        }
        (_, target) => Err(format!(
            "`{name}` on a {} is not supported",
            target.type_name()
        )),
    }
}

/// CEL's `&&`: false wins over an error on either side.
fn and(a: Val<bool>, b: Val<bool>) -> Val<bool> {
    match (a, b) {
        (Val::Const(false), _) | (_, Val::Const(false)) => Val::Const(false),
        (Val::Const(true), other) | (other, Val::Const(true)) => other,
        (Val::Dyn(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |i| match a(i) {
            Ok(true) => b(i),
            Ok(false) => Ok(false),
            Err(err) => match b(i) {
                Ok(false) => Ok(false),
                _ => Err(err),
            },
        })),
    }
}

/// CEL's `||`: true wins over an error on either side.
fn or(a: Val<bool>, b: Val<bool>) -> Val<bool> {
    match (a, b) {
        (Val::Const(true), _) | (_, Val::Const(true)) => Val::Const(true),
        (Val::Const(false), other) | (other, Val::Const(false)) => other,
        (Val::Dyn(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |i| match a(i) {
            Ok(false) => b(i),
            Ok(true) => Ok(true),
            Err(err) => match b(i) {
                Ok(true) => Ok(true),
                _ => Err(err),
            },
        })),
    }
}

fn pick<T: Clone + Send + Sync + 'static>(cond: Val<bool>, a: Val<T>, b: Val<T>) -> Val<T> {
    match cond {
        Val::Const(true) => a,
        Val::Const(false) => b,
        Val::Dyn(cond) => {
            let (a, b) = (a, b);
            Val::Dyn(Box::new(
                move |i| if cond(i)? { a.eval(i) } else { b.eval(i) },
            ))
        }
    }
}

fn conditional(cond: Val<bool>, a: Node, b: Node) -> Result<Node> {
    Ok(match (a, b) {
        (Node::Bool(a), Node::Bool(b)) => Node::Bool(pick(cond, a, b)),
        (Node::Number(a), Node::Number(b)) => Node::Number(match Num::unify(a, b)? {
            Pair::UInt(a, b) => Num::UInt(pick(cond, a, b)),
            Pair::Int(a, b) => Num::Int(pick(cond, a, b)),
            Pair::Decimal(a, b) => Num::Decimal(pick(cond, a, b)),
        }),
        (Node::Address(a), Node::Address(b)) => Node::Address(pick(cond, a, b)),
        (Node::Timestamp(a), Node::Timestamp(b)) => Node::Timestamp(pick(cond, a, b)),
        (Node::Duration(a), Node::Duration(b)) => Node::Duration(pick(cond, a, b)),
        (Node::Fee(a), Node::Fee(b)) => Node::Fee(pick(cond, a, b)),
        (a @ (Node::Fee(_) | Node::Fees(_)), b @ (Node::Fee(_) | Node::Fees(_))) => {
            Node::Fees(pick(cond, a.into_fees()?, b.into_fees()?))
        }
        (Node::Text(a), Node::Text(b)) => Node::Text(match cond {
            Val::Const(true) => a,
            Val::Const(false) => b,
            Val::Dyn(cond) => Text::Dyn(Box::new(
                move |i| if cond(i)? { a.eval(i) } else { b.eval(i) },
            )),
        }),
        (a, b) => {
            return Err(format!(
                "the branches of `?:` are a {} and a {}",
                a.type_name(),
                b.type_name()
            ));
        }
    })
}

/// An address literal in any case.
fn address_literal(text: &str) -> Result<Address> {
    text.parse()
        .map_err(|_| format!("`{text}` is compared with an address but is not one"))
}

fn equals(a: Node, b: Node) -> Result<Val<bool>> {
    fn eq<T: PartialEq + Clone + Send + Sync + 'static>(a: Val<T>, b: Val<T>) -> Result<Val<bool>> {
        map2(a, b, |a, b| Ok(a == b))
    }
    match (a, b) {
        (Node::Bool(a), Node::Bool(b)) => eq(a, b),
        (Node::Number(a), Node::Number(b)) => compare_numbers(a, b, Ordering::is_eq),
        (Node::Address(a), Node::Address(b)) => eq(a, b),
        (Node::Address(a), Node::Text(Text::Const(text)))
        | (Node::Text(Text::Const(text)), Node::Address(a)) => {
            eq(a, Val::Const(address_literal(&text)?))
        }
        (Node::Text(a), Node::Text(b)) => Ok(a.test(b, |a, b| a == b)),
        (Node::Timestamp(a), Node::Timestamp(b)) => eq(a, b),
        (Node::Duration(a), Node::Duration(b)) => eq(a, b),
        (a, b) => Err(format!(
            "cannot compare a {} with a {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn compare(a: Node, b: Node, test: fn(Ordering) -> bool) -> Result<Val<bool>> {
    fn ord<T: PartialOrd + Clone + Send + Sync + 'static>(
        a: Val<T>,
        b: Val<T>,
        test: fn(Ordering) -> bool,
    ) -> Result<Val<bool>> {
        map2(a, b, move |a, b| Ok(a.partial_cmp(&b).is_some_and(test)))
    }
    match (a, b) {
        (Node::Number(a), Node::Number(b)) => compare_numbers(a, b, test),
        (Node::Timestamp(a), Node::Timestamp(b)) => ord(a, b, test),
        (Node::Duration(a), Node::Duration(b)) => ord(a, b, test),
        (Node::Bool(a), Node::Bool(b)) => ord(a, b, test),
        (Node::Text(Text::Const(a)), Node::Text(Text::Const(b))) => {
            Ok(Val::Const(test(a.as_str().cmp(&b))))
        }
        (Node::Text(a), Node::Text(b)) => {
            let a = to_dyn_text(a);
            let b = to_dyn_text(b);
            Ok(Val::Dyn(Box::new(move |i| {
                Ok(test(a(i)?.as_ref().cmp(b(i)?.as_ref())))
            })))
        }
        (a, b) => Err(format!(
            "cannot order a {} and a {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

/// Numbers compare exactly across types: natively when both sides share a
/// type (a constant converts if that keeps its value), else through
/// [`Number::cmp_exact`], so a large amount never fails to convert.
fn compare_numbers(a: Num, b: Num, test: fn(Ordering) -> bool) -> Result<Val<bool>> {
    fn native<T: Ord + Clone + Send + Sync + 'static>(
        a: Val<T>,
        b: Val<T>,
        test: fn(Ordering) -> bool,
    ) -> Result<Val<bool>> {
        map2(a, b, move |a, b| Ok(test(a.cmp(&b))))
    }
    match Num::shared_kind(&a, &b, true) {
        Some(kind) => match Num::pair(a, b, kind)? {
            Pair::UInt(a, b) => native(a, b, test),
            Pair::Int(a, b) => native(a, b, test),
            Pair::Decimal(a, b) => native(a, b, test),
        },
        None => map2(a.into_number()?, b.into_number()?, move |a, b| {
            Ok(test(a.cmp_exact(b)))
        }),
    }
}

/// The larger (or smaller) of two numbers, in their unified type.
fn extreme(a: Num, b: Num, max: bool) -> Result<Num> {
    fn pick<T: Ord + Clone + Send + Sync + 'static>(
        a: Val<T>,
        b: Val<T>,
        max: bool,
    ) -> Result<Val<T>> {
        map2(a, b, move |a, b| Ok(if max { a.max(b) } else { a.min(b) }))
    }
    Ok(match Num::unify(a, b)? {
        Pair::UInt(a, b) => Num::UInt(pick(a, b, max)?),
        Pair::Int(a, b) => Num::Int(pick(a, b, max)?),
        Pair::Decimal(a, b) => Num::Decimal(pick(a, b, max)?),
    })
}

fn to_dyn_text(text: Text) -> crate::node::DynText {
    match text {
        Text::Const(text) => Box::new(move |_| Ok(Cow::Owned(text.clone()))),
        Text::Dyn(f) => f,
    }
}

/// `x in list`: a hash lookup in the list's set of the right type.
fn contains(item: Node, list: Node) -> Result<Val<bool>> {
    let Node::List(set) = list else {
        return Err(format!("`in` needs a list, found a {}", list.type_name()));
    };
    match item {
        Node::Address(address) => address.map(move |address| Ok(set.addresses.contains(&address))),
        Node::Text(Text::Const(text)) => Ok(Val::Const(
            set.texts.contains(&text)
                || text
                    .parse::<Address>()
                    .is_ok_and(|address| set.addresses.contains(&address)),
        )),
        Node::Text(Text::Dyn(text)) => Ok(Val::Dyn(Box::new(move |i| {
            Ok(set.texts.contains(text(i)?.as_ref()))
        }))),
        other => Err(format!("lists hold strings, not a {}", other.type_name())),
    }
}

fn arithmetic(op: &str, a: Node, b: Node) -> Result<Node> {
    Ok(match (op, a, b) {
        (_, Node::Number(a), Node::Number(b)) => Node::Number(match Num::unify(a, b)? {
            Pair::UInt(a, b) => Num::UInt(number_arithmetic(op, a, b)?),
            Pair::Int(a, b) => Num::Int(number_arithmetic(op, a, b)?),
            Pair::Decimal(a, b) => Num::Decimal(number_arithmetic(op, a, b)?),
        }),
        ("_+_", Node::Timestamp(t), Node::Duration(d))
        | ("_+_", Node::Duration(d), Node::Timestamp(t)) => Node::Timestamp(map2(t, d, |t, d| {
            t.checked_add_signed(d).ok_or(EvalError::Overflow)
        })?),
        ("_-_", Node::Timestamp(t), Node::Duration(d)) => Node::Timestamp(map2(t, d, |t, d| {
            t.checked_sub_signed(d).ok_or(EvalError::Overflow)
        })?),
        ("_-_", Node::Timestamp(a), Node::Timestamp(b)) => {
            Node::Duration(map2(a, b, |a, b| Ok(a - b))?)
        }
        ("_+_", Node::Duration(a), Node::Duration(b)) => Node::Duration(map2(a, b, |a, b| {
            a.checked_add(&b).ok_or(EvalError::Overflow)
        })?),
        ("_-_", Node::Duration(a), Node::Duration(b)) => Node::Duration(map2(a, b, |a, b| {
            a.checked_sub(&b).ok_or(EvalError::Overflow)
        })?),
        (_, a, b) => {
            return Err(format!(
                "`{op}` of a {} and a {} is not supported",
                a.type_name(),
                b.type_name()
            ));
        }
    })
}

fn timestamp(arg: Node) -> Result<Node> {
    match arg {
        Node::Text(Text::Const(text)) => DateTime::parse_from_rfc3339(&text)
            .map(|t| Node::Timestamp(Val::Const(t.with_timezone(&Utc))))
            .map_err(|err| format!("`timestamp(\"{text}\")`: {err}")),
        // The view keeps `creationDate` as a timestamp already.
        Node::Timestamp(t) => Ok(Node::Timestamp(t)),
        Node::Number(seconds) => {
            let t = seconds.into_number()?.map(|seconds| {
                let seconds = i64::try_from(seconds.to_int()?).map_err(|_| EvalError::Overflow)?;
                DateTime::from_timestamp(seconds, 0).ok_or(EvalError::Overflow)
            })?;
            Ok(Node::Timestamp(t))
        }
        other => Err(format!(
            "`timestamp` of a {} is not supported",
            other.type_name()
        )),
    }
}

/// A CEL duration such as `24h`, `1h30m` or `-1.5s`.
fn parse_duration(text: &str) -> Result<TimeDelta> {
    let bad = || format!("`duration(\"{text}\")` is not a duration");
    let (negative, mut rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if rest.is_empty() {
        return Err(bad());
    }
    let mut nanos = Decimal::ZERO;
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .ok_or_else(bad)?;
        let number: Decimal = rest[..end].parse().map_err(|_| bad())?;
        rest = &rest[end..];
        let (unit, unit_nanos) = [
            ("ns", 1),
            ("us", 1_000),
            ("ms", 1_000_000),
            ("h", 3_600_000_000_000),
            ("m", 60_000_000_000),
            ("s", 1_000_000_000_i64),
        ]
        .into_iter()
        .find(|(unit, _)| rest.starts_with(unit))
        .ok_or_else(bad)?;
        rest = &rest[unit.len()..];
        nanos = number
            .checked_mul(Decimal::from(unit_nanos))
            .and_then(|n| nanos.checked_add(n))
            .ok_or_else(bad)?;
    }
    let nanos = i64::try_from(nanos.trunc()).map_err(|_| bad())?;
    Ok(TimeDelta::nanoseconds(if negative {
        -nanos
    } else {
        nanos
    }))
}

/// A list literal: fees, or strings for `in`.
fn list_node(elements: &[IdedExpr], scope: &Scope<'_>) -> Result<Node> {
    let nodes = elements
        .iter()
        .map(|element| compile(element, scope))
        .collect::<Result<Vec<_>>>()?;
    if nodes.iter().all(|node| matches!(node, Node::Fee(_))) && !nodes.is_empty() {
        let fees: Vec<Val<_>> = nodes
            .into_iter()
            .map(|node| match node {
                Node::Fee(fee) => fee,
                _ => unreachable!(),
            })
            .collect();
        if fees.iter().all(|fee| matches!(fee, Val::Const(_))) {
            let fees = fees
                .into_iter()
                .map(|fee| match fee {
                    Val::Const(fee) => fee,
                    Val::Dyn(_) => unreachable!(),
                })
                .collect();
            return Ok(Node::Fees(Val::Const(fees)));
        }
        return Ok(Node::Fees(Val::Dyn(Box::new(move |i| {
            fees.iter().map(|fee| fee.eval(i)).collect()
        }))));
    }
    let texts = nodes
        .into_iter()
        .map(|node| match node {
            Node::Text(Text::Const(text)) => Ok(text),
            other => Err(format!(
                "a list literal holds fees or string literals, not a {}",
                other.type_name()
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Node::List(Arc::new(Set::new(
        texts.iter().map(String::as_str),
    ))))
}
