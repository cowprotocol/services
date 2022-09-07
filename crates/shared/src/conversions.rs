use anyhow::{ensure, Result};


use num::{rational::Ratio, BigInt, BigRational};
use primitive_types::U256;

// Convenience:

pub trait RatioExt<T> {
    fn new_checked(numerator: T, denominator: T) -> Result<Ratio<T>>;
}

impl<T: num::Integer + Clone> RatioExt<T> for Ratio<T> {
    fn new_checked(numerator: T, denominator: T) -> Result<Ratio<T>> {
        ensure!(
            !denominator.is_zero(),
            "Cannot create Ratio with 0 denominator"
        );
        Ok(Ratio::new(numerator, denominator))
    }
}

pub trait U256Ext: Sized {
    fn to_big_int(&self) -> BigInt;
    fn to_big_rational(&self) -> BigRational;

    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;
    fn ceil_div(&self, other: &Self) -> Self;
}

impl U256Ext for U256 {
    fn to_big_int(&self) -> BigInt {
        number_conversions::u256_to_big_int(self)
    }
    fn to_big_rational(&self) -> BigRational {
        number_conversions::u256_to_big_rational(self)
    }

    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(1.into())?)?
            .checked_div(*other)
    }
    fn ceil_div(&self, other: &Self) -> Self {
        self.checked_ceil_div(other)
            .expect("ceiling division arithmetic error")
    }
}
