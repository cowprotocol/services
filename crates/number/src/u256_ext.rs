//! Extension trait for U256 arithmetic operations.

use {
    alloy::primitives::U256,
    anyhow::Result,
    num::{BigInt, BigRational, BigUint},
};

/// Extension trait for U256 to add utility methods.
pub trait U256Ext: Sized {
    /// Ceiling division: (self + other - 1) / other
    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;

    /// Ceiling division that panics on error.
    fn ceil_div(&self, other: &Self) -> Self {
        self.checked_ceil_div(other)
            .expect("ceiling division arithmetic error")
    }

    /// Multiply U256 by f64 factor with high precision.
    ///
    /// The factor is first multiplied by a conversion factor to convert it to
    /// an integer, avoiding rounding to 0. The result is then divided by
    /// the conversion factor to convert it back to the original scale.
    ///
    /// The higher the conversion factor (10^18), the higher the precision. For
    /// example, 0.123456789123456789 will be converted to 123456789123456789.
    fn checked_mul_f64(&self, factor: f64) -> Option<Self>;

    /// Convert to BigInt.
    fn to_big_int(&self) -> BigInt;

    /// Convert to BigUint.
    fn to_big_uint(&self) -> BigUint;

    /// Convert to BigRational.
    fn to_big_rational(&self) -> BigRational;

    /// Create from BigInt.
    fn from_big_int(input: &BigInt) -> Result<Self>;

    /// Create from BigUint.
    fn from_big_uint(input: &BigUint) -> Result<Self>;

    /// Create from BigRational.
    fn from_big_rational(value: &BigRational) -> Result<Self>;
}

impl U256Ext for U256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(U256::from(1u64))?)?
            .checked_div(*other)
    }

    fn checked_mul_f64(&self, factor: f64) -> Option<Self> {
        const CONVERSION_FACTOR: f64 = 1_000_000_000_000_000_000.;
        let multiplied = self
            .checked_mul(U256::from(factor * CONVERSION_FACTOR))?
            .checked_div(U256::from(CONVERSION_FACTOR))?;
        Some(multiplied)
    }

    fn to_big_int(&self) -> BigInt {
        BigInt::from_biguint(num::bigint::Sign::Plus, self.to_big_uint())
    }

    fn to_big_uint(&self) -> BigUint {
        BigUint::from_bytes_be(self.to_be_bytes::<32>().as_slice())
    }

    fn to_big_rational(&self) -> BigRational {
        BigRational::new(self.to_big_int(), 1.into())
    }

    fn from_big_int(input: &BigInt) -> Result<Self> {
        anyhow::ensure!(input.sign() != num::bigint::Sign::Minus, "negative");
        Self::from_big_uint(input.magnitude())
    }

    fn from_big_uint(input: &BigUint) -> Result<Self> {
        let bytes = input.to_bytes_be();
        anyhow::ensure!(bytes.len() <= 32, "too large");
        Ok(U256::from_be_slice(&bytes))
    }

    fn from_big_rational(value: &BigRational) -> Result<Self> {
        use num::Zero;
        anyhow::ensure!(*value.denom() != BigInt::zero(), "zero denominator");
        Self::from_big_int(&(value.numer() / value.denom()))
    }
}
