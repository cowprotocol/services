//! Typed compiled expressions: a constant, or a closure over the order.

use {
    crate::{EvalError, order::Input},
    alloy_primitives::Address,
    chrono::{DateTime, TimeDelta, Utc},
    fee_engine::Fee,
    std::{borrow::Cow, collections::HashSet, sync::Arc},
};

/// A closure that computes a value from one order.
pub(crate) type Dyn<T> = Box<dyn Fn(&Input<'_>) -> Result<T, EvalError> + Send + Sync>;

/// A text closure; it may borrow from the order.
pub(crate) type DynText =
    Box<dyn for<'a> Fn(&Input<'a>) -> Result<Cow<'a, str>, EvalError> + Send + Sync>;

/// A value known when the rules load, or computed per order.
pub(crate) enum Val<T> {
    Const(T),
    Dyn(Dyn<T>),
}

impl<T: Clone + Send + Sync + 'static> Val<T> {
    pub(crate) fn eval(&self, input: &Input<'_>) -> Result<T, EvalError> {
        match self {
            Val::Const(value) => Ok(value.clone()),
            Val::Dyn(f) => f(input),
        }
    }

    /// Applies `f`; folds it when the value is constant.
    pub(crate) fn map<R: Clone + Send + Sync + 'static>(
        self,
        f: impl Fn(T) -> Result<R, EvalError> + Send + Sync + 'static,
    ) -> Result<Val<R>, String> {
        Ok(match self {
            Val::Const(value) => Val::Const(f(value).map_err(|err| err.to_string())?),
            Val::Dyn(a) => Val::Dyn(Box::new(move |input| f(a(input)?))),
        })
    }
}

/// Combines two values with `f`; folds it when both are constant.
pub(crate) fn map2<A, B, R>(
    a: Val<A>,
    b: Val<B>,
    f: impl Fn(A, B) -> Result<R, EvalError> + Send + Sync + 'static,
) -> Result<Val<R>, String>
where
    A: Clone + Send + Sync + 'static,
    B: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
{
    Ok(match (a, b) {
        (Val::Const(a), Val::Const(b)) => Val::Const(f(a, b).map_err(|err| err.to_string())?),
        (Val::Const(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |input| f(a.clone(), b(input)?))),
        (Val::Dyn(a), Val::Const(b)) => Val::Dyn(Box::new(move |input| f(a(input)?, b.clone()))),
        (Val::Dyn(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |input| f(a(input)?, b(input)?))),
    })
}

/// Text, known when the rules load or computed per order.
pub(crate) enum Text {
    Const(String),
    Dyn(DynText),
}

impl Text {
    pub(crate) fn eval<'a>(&self, input: &Input<'a>) -> Result<Cow<'a, str>, EvalError> {
        match self {
            Text::Const(text) => Ok(Cow::Owned(text.clone())),
            Text::Dyn(f) => f(input),
        }
    }

    /// Applies a text-to-text function; folds it when the text is constant.
    pub(crate) fn map(self, f: fn(Cow<'_, str>) -> Cow<'_, str>) -> Text {
        match self {
            Text::Const(text) => Text::Const(f(Cow::Owned(text)).into_owned()),
            Text::Dyn(a) => Text::Dyn(Box::new(move |input| Ok(f(a(input)?)))),
        }
    }

    /// A predicate over the text and a second text, such as `==` or
    /// `startsWith`. The constant side is not cloned per order.
    pub(crate) fn test(self, other: Text, f: fn(&str, &str) -> bool) -> Val<bool> {
        match (self, other) {
            (Text::Const(a), Text::Const(b)) => Val::Const(f(&a, &b)),
            (Text::Dyn(a), Text::Const(b)) => {
                Val::Dyn(Box::new(move |input| Ok(f(&a(input)?, &b))))
            }
            (Text::Const(a), Text::Dyn(b)) => {
                Val::Dyn(Box::new(move |input| Ok(f(&a, &b(input)?))))
            }
            (Text::Dyn(a), Text::Dyn(b)) => {
                Val::Dyn(Box::new(move |input| Ok(f(&a(input)?, &b(input)?))))
            }
        }
    }
}

/// The members of a list, split by type so `in` is one hash lookup.
#[derive(Debug, Default)]
pub(crate) struct Set {
    pub(crate) addresses: HashSet<Address>,
    pub(crate) texts: HashSet<String>,
}

impl Set {
    /// Hex members are addresses (any case); the rest are texts as written.
    pub(crate) fn new<'a>(members: impl IntoIterator<Item = &'a str>) -> Self {
        let mut set = Set::default();
        for member in members {
            match member.parse::<Address>() {
                Ok(address) if member.starts_with("0x") => {
                    set.addresses.insert(address);
                }
                _ => {
                    set.texts.insert(member.to_owned());
                }
            }
        }
        set
    }
}

/// Where a field of the order is, for `has`, `.?` and plain reads.
#[derive(Clone, Copy)]
pub(crate) enum Path {
    Object(&'static crate::order::Object),
    Field(&'static crate::order::Field),
}

/// A compiled expression with its type.
pub(crate) enum Node {
    Bool(Val<bool>),
    Number(crate::number::Num),
    Text(Text),
    Address(Val<Address>),
    Timestamp(Val<DateTime<Utc>>),
    Duration(Val<TimeDelta>),
    /// One fee, from a fee function.
    Fee(Val<Fee>),
    /// Several fees, from a list or `no_fee()`.
    Fees(Val<Vec<Fee>>),
    List(Arc<Set>),
    /// `order` or one of its nested objects, read plainly.
    Object(&'static crate::order::Object),
    /// The result of `.?`: a field that may be absent.
    Optional(Path),
}

impl Node {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Node::Bool(_) => "bool",
            Node::Number(_) => "number",
            Node::Text(_) => "string",
            Node::Address(_) => "address",
            Node::Timestamp(_) => "timestamp",
            Node::Duration(_) => "duration",
            Node::Fee(_) => "fee",
            Node::Fees(_) => "list of fees",
            Node::List(_) => "list",
            Node::Object(_) => "object",
            Node::Optional(_) => "optional",
        }
    }

    pub(crate) fn into_bool(self) -> Result<Val<bool>, String> {
        match self {
            Node::Bool(value) => Ok(value),
            other => Err(format!("expected a bool, found a {}", other.type_name())),
        }
    }

    pub(crate) fn into_number(self) -> Result<crate::number::Num, String> {
        match self {
            Node::Number(value) => Ok(value),
            other => Err(format!("expected a number, found a {}", other.type_name())),
        }
    }

    /// Text; an address becomes its lowercase hex.
    pub(crate) fn into_text(self) -> Result<Text, String> {
        match self {
            Node::Text(text) => Ok(text),
            Node::Address(Val::Const(address)) => Ok(Text::Const(hex(address))),
            Node::Address(Val::Dyn(f)) => Ok(Text::Dyn(Box::new(move |input| {
                Ok(Cow::Owned(hex(f(input)?)))
            }))),
            other => Err(format!("expected a string, found a {}", other.type_name())),
        }
    }

    /// Fees as a list: one fee becomes a list of one.
    pub(crate) fn into_fees(self) -> Result<Val<Vec<Fee>>, String> {
        match self {
            Node::Fees(fees) => Ok(fees),
            Node::Fee(fee) => fee.map(|fee| Ok(vec![fee])),
            other => Err(format!("expected fees, found a {}", other.type_name())),
        }
    }
}

fn hex(address: Address) -> String {
    format!("{address:#x}")
}
