//! Exact numbers: `uint` is a `U256`, `int` an `I256`, a number with a
//! fraction a `Decimal`. Mixed operands are promoted, never rounded.

use {
    crate::{
        EvalError,
        node::{Val, map2},
    },
    alloy_primitives::{I256, U256, U512},
    rust_decimal::{
        Decimal,
        prelude::{FromPrimitive, ToPrimitive},
    },
    std::cmp::Ordering,
};

/// A number of any of the three types, for the exact mixed-type paths.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Number {
    UInt(U256),
    Int(I256),
    Decimal(Decimal),
}

impl Number {
    /// `(negative, magnitude, scale)`: the value is `±magnitude / 10^scale`.
    fn parts(self) -> (bool, U512, u32) {
        match self {
            Number::UInt(u) => (false, U512::from(u), 0),
            Number::Int(i) => (i.is_negative(), U512::from(i.unsigned_abs()), 0),
            Number::Decimal(d) => (
                d.is_sign_negative() && !d.is_zero(),
                U512::from(d.mantissa().unsigned_abs()),
                d.scale(),
            ),
        }
    }

    /// Exact order of any two numbers. Magnitudes are at most 2^256 and a
    /// decimal scale at most 28, so the scaled values fit 512 bits.
    pub(crate) fn cmp_exact(self, other: Number) -> Ordering {
        let (a_neg, a, a_scale) = self.parts();
        let (b_neg, b, b_scale) = other.parts();
        let a_zero = a.is_zero();
        let b_zero = b.is_zero();
        let (a_neg, b_neg) = (a_neg && !a_zero, b_neg && !b_zero);
        if a_neg != b_neg {
            return if a_neg {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let ten = U512::from(10);
        let a = a * ten.pow(U512::from(b_scale));
        let b = b * ten.pow(U512::from(a_scale));
        if a_neg { b.cmp(&a) } else { a.cmp(&b) }
    }

    pub(crate) fn to_uint(self) -> Result<U256, EvalError> {
        match self {
            Number::UInt(u) => Ok(u),
            Number::Int(i) => U256::try_from(i).map_err(|_| EvalError::Range("uint")),
            Number::Decimal(d) => d
                .trunc()
                .to_u128()
                .map(U256::from)
                .ok_or(EvalError::Range("uint")),
        }
    }

    pub(crate) fn to_int(self) -> Result<I256, EvalError> {
        match self {
            Number::UInt(u) => I256::try_from(u).map_err(|_| EvalError::Range("int")),
            Number::Int(i) => Ok(i),
            Number::Decimal(d) => Ok(I256::try_from(
                d.trunc().to_i128().expect("a decimal is below 2^96"),
            )
            .expect("an i128 fits 256 bits")),
        }
    }

    pub(crate) fn to_decimal(self) -> Result<Decimal, EvalError> {
        let range = || EvalError::Range("decimal");
        match self {
            Number::UInt(u) => u128::try_from(u)
                .ok()
                .and_then(Decimal::from_u128)
                .ok_or_else(range),
            Number::Int(i) => i128::try_from(i)
                .ok()
                .and_then(Decimal::from_i128)
                .ok_or_else(range),
            Number::Decimal(d) => Ok(d),
        }
    }

    /// Exact conversion, or `None` when the value would change.
    fn convert(self, kind: Kind) -> Option<Number> {
        let converted = match kind {
            Kind::UInt => Number::UInt(self.to_uint().ok()?),
            Kind::Int => Number::Int(self.to_int().ok()?),
            Kind::Decimal => Number::Decimal(self.to_decimal().ok()?),
        };
        (converted.cmp_exact(self) == Ordering::Equal).then_some(converted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    UInt,
    Int,
    Decimal,
}

/// A compiled number of one of the three types.
pub(crate) enum Num {
    UInt(Val<U256>),
    Int(Val<I256>),
    Decimal(Val<Decimal>),
}

/// Two numbers brought to the same type.
pub(crate) enum Pair {
    UInt(Val<U256>, Val<U256>),
    Int(Val<I256>, Val<I256>),
    Decimal(Val<Decimal>, Val<Decimal>),
}

impl Num {
    pub(crate) fn kind(&self) -> Kind {
        match self {
            Num::UInt(_) => Kind::UInt,
            Num::Int(_) => Kind::Int,
            Num::Decimal(_) => Kind::Decimal,
        }
    }

    fn constant(&self) -> Option<Number> {
        match self {
            Num::UInt(Val::Const(u)) => Some(Number::UInt(*u)),
            Num::Int(Val::Const(i)) => Some(Number::Int(*i)),
            Num::Decimal(Val::Const(d)) => Some(Number::Decimal(*d)),
            _ => None,
        }
    }

    fn from_number(number: Number) -> Num {
        match number {
            Number::UInt(u) => Num::UInt(Val::Const(u)),
            Number::Int(i) => Num::Int(Val::Const(i)),
            Number::Decimal(d) => Num::Decimal(Val::Const(d)),
        }
    }

    /// The number as a [`Number`], for the mixed-type paths.
    pub(crate) fn into_number(self) -> Result<Val<Number>, String> {
        match self {
            Num::UInt(v) => v.map(|u| Ok(Number::UInt(u))),
            Num::Int(v) => v.map(|i| Ok(Number::Int(i))),
            Num::Decimal(v) => v.map(|d| Ok(Number::Decimal(d))),
        }
    }

    /// Converts to `kind`: exactly at compile time for a constant (else a
    /// compile error), checked per order otherwise.
    pub(crate) fn into_kind(self, kind: Kind) -> Result<Num, String> {
        if self.kind() == kind {
            return Ok(self);
        }
        if let Some(number) = self.constant() {
            return number
                .convert(kind)
                .map(Num::from_number)
                .ok_or_else(|| format!("{number:?} is not exactly a {kind:?}"));
        }
        let number = self.into_number()?;
        Ok(match kind {
            Kind::UInt => Num::UInt(number.map(Number::to_uint)?),
            Kind::Int => Num::Int(number.map(Number::to_int)?),
            Kind::Decimal => Num::Decimal(number.map(Number::to_decimal)?),
        })
    }

    pub(crate) fn into_decimal(self) -> Result<Val<Decimal>, String> {
        match self.into_kind(Kind::Decimal)? {
            Num::Decimal(d) => Ok(d),
            _ => unreachable!(),
        }
    }

    /// The type both operands share without changing a value. A constant
    /// takes the other side's type if it converts exactly; a decimal constant
    /// only if `decimal_to_integer` (so `sellAmount / 1e20` stays decimal).
    pub(crate) fn shared_kind(a: &Num, b: &Num, decimal_to_integer: bool) -> Option<Kind> {
        let (ka, kb) = (a.kind(), b.kind());
        let adopts = |constant: Option<Number>, from: Kind, to: Kind| {
            (decimal_to_integer || from != Kind::Decimal)
                && constant.is_some_and(|n| n.convert(to).is_some())
        };
        if ka == kb || adopts(b.constant(), kb, ka) {
            Some(ka)
        } else if adopts(a.constant(), ka, kb) {
            Some(kb)
        } else {
            None
        }
    }

    /// Brings both operands to one type: the shared one, else the wider of
    /// the two (uint < int < decimal), converted and checked per order.
    pub(crate) fn unify(a: Num, b: Num) -> Result<Pair, String> {
        let kind = Num::shared_kind(&a, &b, false).unwrap_or(
            if a.kind() == Kind::Decimal || b.kind() == Kind::Decimal {
                Kind::Decimal
            } else {
                Kind::Int
            },
        );
        Num::pair(a, b, kind)
    }

    pub(crate) fn pair(a: Num, b: Num, kind: Kind) -> Result<Pair, String> {
        Ok(match (a.into_kind(kind)?, b.into_kind(kind)?) {
            (Num::UInt(a), Num::UInt(b)) => Pair::UInt(a, b),
            (Num::Int(a), Num::Int(b)) => Pair::Int(a, b),
            (Num::Decimal(a), Num::Decimal(b)) => Pair::Decimal(a, b),
            _ => unreachable!("both converted to {kind:?}"),
        })
    }
}

/// Integer and decimal arithmetic, all checked.
pub(crate) trait Arith: Copy + Ord + Send + Sync + 'static {
    fn add(self, other: Self) -> Option<Self>;
    fn sub(self, other: Self) -> Option<Self>;
    fn mul(self, other: Self) -> Option<Self>;
    fn div(self, other: Self) -> Option<Self>;
    fn rem(self, other: Self) -> Option<Self>;
}

macro_rules! arith {
    ($t:ty) => {
        impl Arith for $t {
            fn add(self, other: Self) -> Option<Self> {
                self.checked_add(other)
            }

            fn sub(self, other: Self) -> Option<Self> {
                self.checked_sub(other)
            }

            fn mul(self, other: Self) -> Option<Self> {
                self.checked_mul(other)
            }

            fn div(self, other: Self) -> Option<Self> {
                self.checked_div(other)
            }

            fn rem(self, other: Self) -> Option<Self> {
                self.checked_rem(other)
            }
        }
    };
}

arith!(U256);
arith!(I256);
arith!(Decimal);

/// `a <op> b` on one type; overflow, underflow and division by zero are
/// errors, so the rule is skipped.
pub(crate) fn arithmetic<T: Arith>(op: &str, a: Val<T>, b: Val<T>) -> Result<Val<T>, String> {
    let f: fn(T, T) -> Option<T> = match op {
        "_+_" => T::add,
        "_-_" => T::sub,
        "_*_" => T::mul,
        "_/_" => T::div,
        _ => T::rem,
    };
    map2(a, b, move |a, b| f(a, b).ok_or(EvalError::Arithmetic))
}
