//! Typed compiled expressions: a constant, or a closure over the order.
//!
//! A [`Node`] is a compiled expression with its type. Most hold a [`Val`];
//! text has its own [`Text`], because it may borrow from the order.

use {
    super::number::Num,
    crate::{
        Fee,
        RuleError,
        order_view::{Object, OrderView},
    },
    alloy_primitives::{Address, map::AddressSet},
    std::{borrow::Cow, collections::HashSet, sync::Arc},
};

/// What a compiled value must be: a constant is cloned per order, and the
/// rule set is shared between threads.
pub(crate) trait Value: Clone + Send + Sync + 'static {}

impl<T: Clone + Send + Sync + 'static> Value for T {}

/// A closure that computes a value from one order.
pub(crate) type Dyn<T> = Box<dyn Fn(&OrderView<'_>) -> Result<T, RuleError> + Send + Sync>;

/// A text closure; it may borrow from the order.
pub(crate) type DynText =
    Box<dyn for<'a> Fn(&OrderView<'a>) -> Result<Cow<'a, str>, RuleError> + Send + Sync>;

/// A value known when the rules load, or computed per order.
pub(crate) enum Val<T> {
    Const(T),
    Dyn(Dyn<T>),
}

impl<T: Value> Val<T> {
    /// The value for one order.
    pub(crate) fn eval(&self, view: &OrderView<'_>) -> Result<T, RuleError> {
        match self {
            Val::Const(value) => Ok(value.clone()),
            Val::Dyn(f) => f(view),
        }
    }

    /// Applies `f`; folds it when the value is constant.
    pub(crate) fn map<R: Value>(
        self,
        f: impl Fn(T) -> Result<R, RuleError> + Send + Sync + 'static,
    ) -> Result<Val<R>, String> {
        Ok(match self {
            Val::Const(value) => Val::Const(f(value).map_err(|err| err.to_string())?),
            Val::Dyn(a) => Val::Dyn(Box::new(move |view| f(a(view)?))),
        })
    }
}

/// Combines two values with `f`; folds it when both are constant.
pub(crate) fn map2<A: Value, B: Value, R: Value>(
    a: Val<A>,
    b: Val<B>,
    f: impl Fn(A, B) -> Result<R, RuleError> + Send + Sync + 'static,
) -> Result<Val<R>, String> {
    Ok(match (a, b) {
        (Val::Const(a), Val::Const(b)) => Val::Const(f(a, b).map_err(|err| err.to_string())?),
        (Val::Const(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |view| f(a.clone(), b(view)?))),
        (Val::Dyn(a), Val::Const(b)) => Val::Dyn(Box::new(move |view| f(a(view)?, b.clone()))),
        (Val::Dyn(a), Val::Dyn(b)) => Val::Dyn(Box::new(move |view| f(a(view)?, b(view)?))),
    })
}

/// Text, known when the rules load or computed per order.
pub(crate) enum Text {
    Const(String),
    Dyn(DynText),
}

impl Text {
    /// The text for one order; a constant is cloned.
    pub(crate) fn eval<'a>(&self, view: &OrderView<'a>) -> Result<Cow<'a, str>, RuleError> {
        match self {
            Text::Const(text) => Ok(Cow::Owned(text.clone())),
            Text::Dyn(f) => f(view),
        }
    }

    /// A predicate over the text and a second text, such as `==`. The
    /// constant side is not cloned per order.
    pub(crate) fn test(
        self,
        other: Text,
        f: impl Fn(&str, &str) -> bool + Send + Sync + 'static,
    ) -> Val<bool> {
        match (self, other) {
            (Text::Const(a), Text::Const(b)) => Val::Const(f(&a, &b)),
            (Text::Dyn(a), Text::Const(b)) => Val::Dyn(Box::new(move |view| Ok(f(&a(view)?, &b)))),
            (Text::Const(a), Text::Dyn(b)) => Val::Dyn(Box::new(move |view| Ok(f(&a, &b(view)?)))),
            (Text::Dyn(a), Text::Dyn(b)) => {
                Val::Dyn(Box::new(move |view| Ok(f(&a(view)?, &b(view)?))))
            }
        }
    }
}

/// The members of a list, split by type so `in` is one hash lookup.
#[derive(Debug, Default)]
pub(crate) struct Set {
    pub(crate) addresses: AddressSet,
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

/// A compiled expression with its type.
pub(crate) enum Node {
    Bool(Val<bool>),
    Number(Num),
    Text(Text),
    Address(Val<Address>),
    /// One fee, from a fee function.
    Fee(Val<Fee>),
    /// Several fees, from a list or `no_fee()`.
    Fees(Val<Vec<Fee>>),
    List(Arc<Set>),
    /// `order` or one of its nested objects.
    Object(&'static Object),
}

impl Node {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Node::Bool(_) => "bool",
            Node::Number(_) => "number",
            Node::Text(_) => "string",
            Node::Address(_) => "address",
            Node::Fee(_) => "fee",
            Node::Fees(_) => "list of fees",
            Node::List(_) => "list",
            Node::Object(_) => "object",
        }
    }

    pub(crate) fn into_bool(self) -> Result<Val<bool>, String> {
        match self {
            Node::Bool(value) => Ok(value),
            other => Err(format!("expected a bool, found a {}", other.type_name())),
        }
    }

    pub(crate) fn into_number(self) -> Result<Num, String> {
        match self {
            Node::Number(value) => Ok(value),
            other => Err(format!("expected a number, found a {}", other.type_name())),
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
