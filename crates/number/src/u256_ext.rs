//! Extension trait for U256 arithmetic operations.

use {
    alloy::primitives::U256,
    num::{BigInt, BigRational, BigUint, One},
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

    /// Multiply U256 by f64 factor using a conversion factor approach.
    ///
    /// This method converts the factor to a scaled integer by multiplying it by
    /// 10^18, then performs integer arithmetic: `(self * scaled_factor) /
    /// 10^18`.
    ///
    /// This approach preserves precision for factors that, when multiplied by
    /// 10^18, result in values that can be accurately represented in f64
    /// (up to ~9e15).
    ///
    /// We avoid BigRational here because it preserves binary-f64 semantics
    /// and diverges from decimal-intent inputs (e.g., config values).
    ///
    /// Returns `None` if:
    /// - The factor is negative, NaN, or infinity
    /// - The intermediate multiplication would overflow U256
    fn checked_mul_f64(&self, factor: f64) -> Option<Self>;

    /// Convert to BigRational.
    fn to_big_rational(&self) -> BigRational;

    /// Create from BigInt.
    fn from_big_int(input: &BigInt) -> Option<Self>;

    /// Create from BigRational.
    fn from_big_rational(value: &BigRational) -> Option<Self> {
        use num::Zero;
        if value.denom().is_zero() {
            return None;
        }
        Self::from_big_int(&(value.numer() / value.denom()))
    }
}

impl U256Ext for U256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        (!other.is_zero()).then(|| self.div_ceil(*other))
    }

    fn checked_mul_f64(&self, factor: f64) -> Option<Self> {
        if !factor.is_finite() || factor.is_sign_negative() {
            return None;
        }

        // Special case: multiplication by 1.0 is identity
        // This avoids intermediate overflow when multiplying large values
        if factor.is_one() {
            return Some(*self);
        }

        // Scale factor to preserve precision: factor * 10^18
        const SCALE_F64: f64 = 1_000_000_000_000_000_000.0;
        const SCALE_U128: u128 = 1_000_000_000_000_000_000;
        let scaled_factor = factor * SCALE_F64;

        // Convert scaled factor to U256
        let scaled_factor_u256 = if scaled_factor <= u128::MAX as f64 {
            // For values that fit in u128, convert directly
            U256::from(scaled_factor)
        } else {
            // For larger values, use BigUint's f64 conversion
            use num::FromPrimitive;
            let scaled_factor_big = BigUint::from_f64(scaled_factor)?;
            U256::try_from(&scaled_factor_big).ok()?
        };

        // Perform: (self * scaled_factor_u256) / SCALE
        let result = self.checked_mul(scaled_factor_u256)?;
        result.checked_div(U256::from(SCALE_U128))
    }

    fn to_big_rational(&self) -> BigRational {
        BigRational::new(self.into(), 1.into())
    }

    fn from_big_int(input: &BigInt) -> Option<Self> {
        U256::try_from(input).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checked_ceil_div() {
        // Exact division
        assert_eq!(
            U256::from(10u64).checked_ceil_div(&U256::from(2u64)),
            Some(U256::from(5u64))
        );

        // Ceiling needed: 10 / 3 = 3.33... -> 4
        assert_eq!(
            U256::from(10u64).checked_ceil_div(&U256::from(3u64)),
            Some(U256::from(4u64))
        );

        // Ceiling needed: 7 / 2 = 3.5 -> 4
        assert_eq!(
            U256::from(7u64).checked_ceil_div(&U256::from(2u64)),
            Some(U256::from(4u64))
        );

        // 1 / 2 = 0.5 -> 1
        assert_eq!(
            U256::from(1u64).checked_ceil_div(&U256::from(2u64)),
            Some(U256::from(1u64))
        );

        // Division by 1
        assert_eq!(
            U256::from(42u64).checked_ceil_div(&U256::from(1u64)),
            Some(U256::from(42u64))
        );

        // Zero divided by anything (non-zero)
        assert_eq!(
            U256::from(0u64).checked_ceil_div(&U256::from(5u64)),
            Some(U256::from(0u64))
        );

        // Division by zero returns None
        assert_eq!(U256::from(10u64).checked_ceil_div(&U256::ZERO), None);

        // Large number division
        let large = U256::from(1_000_000_000_000_000_000u64); // 1e18
        let divisor = U256::from(3u64);
        // 1e18 / 3 = 333,333,333,333,333,333.33... -> 333,333,333,333,333,334
        assert_eq!(
            large.checked_ceil_div(&divisor),
            Some(U256::from(333_333_333_333_333_334u64))
        );
    }

    #[test]
    fn test_checked_mul_f64() {
        // Realistic enough values
        let value = U256::from(25_000_000_000_000_000_000u128); // 25 ether
        let result = value.checked_mul_f64(0.2).unwrap();
        assert_eq!(
            result,
            U256::from(5_000_000_000_000_000_000u128),
            "25 ether * 0.2 must be exact"
        );

        // Basic functionality
        assert_eq!(
            U256::from(100u64).checked_mul_f64(0.5),
            Some(U256::from(50u64))
        );

        // Multiply by 1.0
        assert_eq!(
            U256::from(12345u64).checked_mul_f64(1.0),
            Some(U256::from(12345u64))
        );

        // Multiply by 0.0
        assert_eq!(U256::from(12345u64).checked_mul_f64(0.0), Some(U256::ZERO));

        // Zero multiplied by any factor
        assert_eq!(U256::ZERO.checked_mul_f64(123.456), Some(U256::ZERO));

        // Negative factor returns None
        assert_eq!(U256::from(100u64).checked_mul_f64(-1.0), None);

        // NaN returns None
        assert_eq!(U256::from(100u64).checked_mul_f64(f64::NAN), None);

        // Infinity returns None
        assert_eq!(U256::from(100u64).checked_mul_f64(f64::INFINITY), None);
        assert_eq!(U256::from(100u64).checked_mul_f64(f64::NEG_INFINITY), None);

        // Test with exact f64 representation: 0.125 = 1/8
        let value = U256::from(1_000_000_000u64); // 1 billion
        let result = value.checked_mul_f64(0.125).unwrap();
        // 1_000_000_000 * 0.125 = 125_000_000
        assert_eq!(result, U256::from(125_000_000u64));

        // Test with 0.25 = 1/4
        let value = U256::from(8_888_888u64);
        let result = value.checked_mul_f64(0.25).unwrap();
        // 8_888_888 * 0.25 = 2_222_222
        assert_eq!(result, U256::from(2_222_222u64));

        // Test with very small exact value
        let value = U256::from(1_000_000_000_000_000_000u64); // 1e18
        let result = value.checked_mul_f64(0.00390625).unwrap(); // 1/256
        // 1e18 / 256 = 3906250000000000
        assert_eq!(result, U256::from(3_906_250_000_000_000u64));

        // Multiplying a large U256 by a large factor should overflow and return None
        let max_u256 = U256::MAX;
        assert_eq!(max_u256.checked_mul_f64(1.1), None);
    }
}
