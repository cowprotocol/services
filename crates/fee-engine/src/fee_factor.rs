//! A share of an amount, such as an order's volume, as an exact decimal.

use {
    crate::FactorError,
    rust_decimal::{
        Decimal,
        prelude::{FromPrimitive, ToPrimitive},
    },
    std::fmt,
};

/// A share in [0, 1). The range is checked when the factor is made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeeFactor(Decimal);

impl FeeFactor {
    pub const ZERO: Self = Self(Decimal::ZERO);

    pub fn get(self) -> Decimal {
        self.0
    }

    /// The nearest double, for hosts that compute fees in `f64`.
    pub fn to_f64(self) -> f64 {
        self.0.to_f64().expect("a decimal in [0, 1) is a double")
    }
}

impl TryFrom<Decimal> for FeeFactor {
    type Error = FactorError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        if (Decimal::ZERO..Decimal::ONE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(FactorError::OutOfRange)
        }
    }
}

/// The shortest decimal that rounds to `value`: 0.3, not the binary fraction
/// nearest to it.
impl TryFrom<f64> for FeeFactor {
    type Error = FactorError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Decimal::from_f64(value)
            .ok_or(FactorError::OutOfRange)?
            .try_into()
    }
}

impl fmt::Display for FeeFactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_the_range() {
        assert_eq!(FeeFactor::try_from(0.).unwrap(), FeeFactor::ZERO);
        assert_eq!(FeeFactor::try_from(0.3).unwrap().get(), Decimal::new(3, 1));
        for value in [-0.1, 1., 1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                FeeFactor::try_from(value),
                Err(FactorError::OutOfRange),
                "{value}"
            );
        }
    }
}
