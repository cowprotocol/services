//! Compiles a rule expression to a typed closure over the order.
//!
//! The `cel` crate parses; [`expression`] walks the tree once, from the
//! leaves up. Each part becomes a [`Node`]: its type, and a constant or a
//! closure. A part that reads nothing from the order is computed here, once.
//! A wrong type, an unknown name or a constant that fails is a compile
//! error, so only failures that depend on the order are left for run time.
//!
//! The README lists the supported subset of CEL; anything else (macros
//! but `has`, functions on a value, maps, indexing) is a compile error.

pub(crate) mod node;
pub(crate) mod number;

use {
    self::{
        node::{Node, Set, Text, Val, Value, map2},
        number::{Num, Pair, arithmetic as number_arithmetic},
    },
    crate::{
        Fee,
        RuleError,
        fee,
        order_view::{Field, FieldKind, Getter, ORDER, OrderView},
    },
    alloy_primitives::{Address, I256, U256},
    cel::common::ast::{CallExpr, Expr, IdedExpr, LiteralValue, SelectExpr, SourceInfo},
    rust_decimal::{Decimal, prelude::FromPrimitive},
    std::{borrow::Cow, cmp::Ordering, collections::HashMap, sync::Arc},
};

/// The names a rule can use besides `order`.
struct Scope<'s> {
    lists: &'s HashMap<String, Arc<Set>>,
    /// Where each literal is in the source, to read it back exactly.
    source: &'s SourceInfo,
}

/// The error is the message of a `CompileError::Expression`.
type Result<T> = std::result::Result<T, String>;

/// Parses `source` and compiles it; literals are read back from the source.
pub(crate) fn expression(
    env: &cel::Env,
    source: &str,
    lists: &HashMap<String, Arc<Set>>,
) -> Result<Node> {
    let program = env.compile(source).map_err(|err| err.to_string())?;
    let scope = Scope {
        lists,
        source: program.source_info(),
    };
    compile(program.expression(), &scope)
}

fn compile(expr: &IdedExpr, scope: &Scope<'_>) -> Result<Node> {
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

/// `order` or the name of a list.
fn ident(name: &str, scope: &Scope<'_>) -> Result<Node> {
    if name == "order" {
        return Ok(Node::Object(&ORDER));
    }
    scope
        .lists
        .get(name)
        .map(|set| Node::List(set.clone()))
        .ok_or_else(|| format!("unknown name `{name}`"))
}

/// `a.b` and `has(a.b)`.
fn select_node(select: &SelectExpr, scope: &Scope<'_>) -> Result<Node> {
    let name = &select.field;
    let object = match compile(&select.operand, scope)? {
        Node::Object(object) => object,
        other => return Err(format!("a {} has no field `{name}`", other.type_name())),
    };
    let field = object
        .field(name)
        .ok_or_else(|| format!("`{}` has no field `{name}`", object.name))?;
    if select.test {
        return Ok(Node::Bool(present(field)));
    }
    Ok(match field.kind {
        FieldKind::Object(object) => Node::Object(object),
        FieldKind::Value(getter) => read(getter, field.name),
    })
}

/// `has(a.b)`: whether the order has the field.
fn present(field: &'static Field) -> Val<bool> {
    match field.kind {
        FieldKind::Object(object) => Val::Dyn(Box::new(|view| Ok((object.present)(view)))),
        FieldKind::Value(getter) => Val::Dyn(Box::new(move |view| Ok(getter.present(view)))),
    }
}

/// A plain read: an absent field is an error, so the rule is skipped.
fn read(getter: Getter, name: &'static str) -> Node {
    fn required<T: Value>(get: fn(&OrderView<'_>) -> Option<T>, name: &'static str) -> Val<T> {
        Val::Dyn(Box::new(move |view| {
            get(view).ok_or(RuleError::Missing(name))
        }))
    }
    match getter {
        Getter::Address(get) => Node::Address(required(get, name)),
        Getter::UInt(get) => Node::Number(Num::UInt(required(get, name))),
        Getter::Int(get) => Node::Number(Num::Int(required(get, name))),
        Getter::Bool(get) => Node::Bool(required(get, name)),
        Getter::Text(get) => Node::Text(Text::Dyn(Box::new(move |view| {
            get(view).map(Cow::Borrowed).ok_or(RuleError::Missing(name))
        }))),
    }
}

/// The arguments of a call, compiled; an error unless there are `N`.
fn arguments<const N: usize>(
    name: &str,
    args: &[IdedExpr],
    scope: &Scope<'_>,
) -> Result<[Node; N]> {
    args.iter()
        .map(|arg| compile(arg, scope))
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| format!("`{name}` takes {N} arguments, not {}", args.len()))
}

/// An operator or a function. The parser gives operators names such as
/// `_&&_` and `-_`. No function is called on a value (`a.f(b)`).
fn call_node(call: &CallExpr, scope: &Scope<'_>) -> Result<Node> {
    let name = call.func_name.as_str();
    if call.target.is_some() {
        return Err(format!("unknown function `{name}`"));
    }
    let args = call.args.as_slice();
    match name {
        "_?._" => Err("`.?` is not supported: guard an optional field with `has(...)`".into()),
        "_&&_" => {
            let [a, b] = arguments(name, args, scope)?;
            Ok(Node::Bool(and(a.into_bool()?, b.into_bool()?)))
        }
        "_||_" => {
            let [a, b] = arguments(name, args, scope)?;
            Ok(Node::Bool(or(a.into_bool()?, b.into_bool()?)))
        }
        "!_" => {
            let [a] = arguments(name, args, scope)?;
            Ok(Node::Bool(a.into_bool()?.map(|a| Ok(!a))?))
        }
        "_?_:_" => {
            let [cond, a, b] = arguments(name, args, scope)?;
            conditional(cond.into_bool()?, a, b)
        }
        "_==_" => {
            let [a, b] = arguments(name, args, scope)?;
            Ok(Node::Bool(equals(a, b)?))
        }
        "_!=_" => {
            let [a, b] = arguments(name, args, scope)?;
            Ok(Node::Bool(equals(a, b)?.map(|equal| Ok(!equal))?))
        }
        "_<_" | "_<=_" | "_>_" | "_>=_" => {
            let [a, b] = arguments(name, args, scope)?;
            let test: fn(Ordering) -> bool = match name {
                "_<_" => Ordering::is_lt,
                "_<=_" => Ordering::is_le,
                "_>_" => Ordering::is_gt,
                _ => Ordering::is_ge,
            };
            compare(a, b, test).map(Node::Bool)
        }
        "@in" => {
            let [item, list] = arguments(name, args, scope)?;
            contains(item, list).map(Node::Bool)
        }
        "_+_" | "_-_" | "_*_" | "_/_" | "_%_" => {
            let [a, b] = arguments(name, args, scope)?;
            arithmetic(name, a, b)
        }
        "-_" => {
            let [a] = arguments(name, args, scope)?;
            negate(a)
        }
        "no_fee" => {
            let [] = arguments(name, args, scope)?;
            Ok(Node::Fees(Val::Const(Vec::new())))
        }
        _ => {
            let numbers = || {
                args.iter()
                    .map(|arg| compile(arg, scope)?.into_number())
                    .collect()
            };
            fee::call(name, numbers)?
                .map(Node::Fee)
                .ok_or_else(|| format!("unknown function `{name}`"))
        }
    }
}

/// CEL's `&&`: false wins over an error on either side.
fn and(a: Val<bool>, b: Val<bool>) -> Val<bool> {
    match (a, b) {
        (Val::Const(false), _) | (_, Val::Const(false)) => Val::Const(false),
        (Val::Const(true), other) | (other, Val::Const(true)) => other,
        (Val::Dyn(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |view| match a(view) {
            Ok(true) => b(view),
            Ok(false) => Ok(false),
            Err(err) => match b(view) {
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
        (Val::Dyn(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |view| match a(view) {
            Ok(false) => b(view),
            Ok(true) => Ok(true),
            Err(err) => match b(view) {
                Ok(true) => Ok(true),
                _ => Err(err),
            },
        })),
    }
}

/// `cond ? a : b` on one type: only the branch taken runs.
fn pick<T: Value>(cond: Val<bool>, a: Val<T>, b: Val<T>) -> Val<T> {
    match cond {
        Val::Const(true) => a,
        Val::Const(false) => b,
        Val::Dyn(cond) => Val::Dyn(Box::new(move |view| {
            if cond(view)? {
                a.eval(view)
            } else {
                b.eval(view)
            }
        })),
    }
}

/// `cond ? a : b`: both branches must have one type. A fee and a list of
/// fees give a list.
fn conditional(cond: Val<bool>, a: Node, b: Node) -> Result<Node> {
    Ok(match (a, b) {
        (Node::Bool(a), Node::Bool(b)) => Node::Bool(pick(cond, a, b)),
        (Node::Number(a), Node::Number(b)) => Node::Number(match Num::unify(a, b)? {
            Pair::UInt(a, b) => Num::UInt(pick(cond, a, b)),
            Pair::Int(a, b) => Num::Int(pick(cond, a, b)),
            Pair::Decimal(a, b) => Num::Decimal(pick(cond, a, b)),
        }),
        (Node::Address(a), Node::Address(b)) => Node::Address(pick(cond, a, b)),
        (Node::Fee(a), Node::Fee(b)) => Node::Fee(pick(cond, a, b)),
        (a @ (Node::Fee(_) | Node::Fees(_)), b @ (Node::Fee(_) | Node::Fees(_))) => {
            Node::Fees(pick(cond, a.into_fees()?, b.into_fees()?))
        }
        (Node::Text(a), Node::Text(b)) => Node::Text(match cond {
            Val::Const(true) => a,
            Val::Const(false) => b,
            Val::Dyn(cond) => Text::Dyn(Box::new(move |view| {
                if cond(view)? {
                    a.eval(view)
                } else {
                    b.eval(view)
                }
            })),
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

/// `a == b` on one type. An address also compares with a string literal.
fn equals(a: Node, b: Node) -> Result<Val<bool>> {
    fn eq<T: Value + PartialEq>(a: Val<T>, b: Val<T>) -> Result<Val<bool>> {
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
        (a, b) => Err(format!(
            "cannot compare a {} with a {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

/// `<`, `<=`, `>` and `>=` on one type; `test` says which orders hold.
fn compare(a: Node, b: Node, test: fn(Ordering) -> bool) -> Result<Val<bool>> {
    fn ord<T: Value + Ord>(a: Val<T>, b: Val<T>, test: fn(Ordering) -> bool) -> Result<Val<bool>> {
        map2(a, b, move |a, b| Ok(test(a.cmp(&b))))
    }
    match (a, b) {
        (Node::Number(a), Node::Number(b)) => compare_numbers(a, b, test),
        (Node::Bool(a), Node::Bool(b)) => ord(a, b, test),
        (Node::Text(a), Node::Text(b)) => Ok(a.test(b, move |a, b| test(a.cmp(b)))),
        (a, b) => Err(format!(
            "cannot order a {} and a {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

/// Numbers compare exactly across types: natively when both sides share a
/// type (a constant converts if that keeps its value), else through
/// [`number::Number::cmp_exact`], so a large amount never fails to convert.
fn compare_numbers(a: Num, b: Num, test: fn(Ordering) -> bool) -> Result<Val<bool>> {
    fn native<T: Value + Ord>(
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

/// `x in list`: a hash lookup in the list's set of the right type.
fn contains(item: Node, list: Node) -> Result<Val<bool>> {
    let Node::List(set) = list else {
        return Err(format!("`in` needs a list, found a {}", list.type_name()));
    };
    match item {
        // A scan of a few addresses is faster than a hash lookup.
        Node::Address(address) if set.addresses.len() <= 4 => {
            let members: Vec<_> = set.addresses.iter().copied().collect();
            address.map(move |address| Ok(members.contains(&address)))
        }
        Node::Address(address) => address.map(move |address| Ok(set.addresses.contains(&address))),
        Node::Text(Text::Const(text)) => Ok(Val::Const(
            set.texts.contains(&text)
                || text
                    .parse::<Address>()
                    .is_ok_and(|address| set.addresses.contains(&address)),
        )),
        Node::Text(Text::Dyn(text)) => Ok(Val::Dyn(Box::new(move |view| {
            Ok(set.texts.contains(text(view)?.as_ref()))
        }))),
        other => Err(format!("lists hold strings, not a {}", other.type_name())),
    }
}

/// `+ - * / %` on two numbers.
fn arithmetic(op: &str, a: Node, b: Node) -> Result<Node> {
    let (a, b) = match (a, b) {
        (Node::Number(a), Node::Number(b)) => (a, b),
        (a, b) => {
            return Err(format!(
                "`{op}` of a {} and a {} is not supported",
                a.type_name(),
                b.type_name()
            ));
        }
    };
    Ok(Node::Number(match Num::unify(a, b)? {
        Pair::UInt(a, b) => Num::UInt(number_arithmetic(op, a, b)?),
        Pair::Int(a, b) => Num::Int(number_arithmetic(op, a, b)?),
        Pair::Decimal(a, b) => Num::Decimal(number_arithmetic(op, a, b)?),
    }))
}

/// `-x` of a number. An integer becomes an `int`.
fn negate(node: Node) -> Result<Node> {
    Ok(Node::Number(match node.into_number()? {
        Num::Decimal(d) => Num::Decimal(d.map(|d| Ok(-d))?),
        integer => Num::Int(
            integer
                .into_int()?
                .map(|i| i.checked_neg().ok_or(RuleError::Arithmetic))?,
        ),
    }))
}

/// A list literal: fees, or string literals for `in`.
fn list_node(elements: &[IdedExpr], scope: &Scope<'_>) -> Result<Node> {
    let mixed = |node: &Node| {
        format!(
            "a list literal holds only fees or only string literals, found a {}",
            node.type_name()
        )
    };
    let nodes = elements
        .iter()
        .map(|element| compile(element, scope))
        .collect::<Result<Vec<_>>>()?;
    if !matches!(nodes.first(), Some(Node::Fee(_))) {
        let texts = nodes
            .into_iter()
            .map(|node| match node {
                Node::Text(Text::Const(text)) => Ok(text),
                other => Err(mixed(&other)),
            })
            .collect::<Result<Vec<_>>>()?;
        return Ok(Node::List(Arc::new(Set::new(
            texts.iter().map(String::as_str),
        ))));
    }
    let fees = nodes
        .into_iter()
        .map(|node| match node {
            Node::Fee(fee) => Ok(fee),
            other => Err(mixed(&other)),
        })
        .collect::<Result<Vec<_>>>()?;
    let constant: Option<Vec<Fee>> = fees
        .iter()
        .map(|fee| match fee {
            Val::Const(fee) => Some(*fee),
            Val::Dyn(_) => None,
        })
        .collect();
    Ok(Node::Fees(match constant {
        Some(fees) => Val::Const(fees),
        None => Val::Dyn(Box::new(move |view| {
            let mut list = Vec::with_capacity(fees.len());
            for fee in &fees {
                list.push(fee.eval(view)?);
            }
            Ok(list)
        })),
    }))
}
