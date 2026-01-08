//! Extension trait for U256 arithmetic operations.

use {
    alloy::primitives::U256 as AlloyU256,
    num::{BigInt, BigRational, BigUint, Signed},
    primitive_types::U256 as PrimitiveU256,
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
    /// Returns `None` if:
    /// - The factor is negative, NaN, or infinity
    /// - The intermediate multiplication would overflow U256
    fn checked_mul_f64(&self, factor: f64) -> Option<Self>;

    /// Convert to BigInt.
    fn to_big_int(&self) -> BigInt;

    /// Convert to BigUint.
    fn to_big_uint(&self) -> BigUint;

    /// Convert to BigRational.
    fn to_big_rational(&self) -> BigRational;

    /// Create from BigInt.
    fn from_big_int(input: &BigInt) -> Option<Self>;

    /// Create from BigUint.
    fn from_big_uint(input: &BigUint) -> Option<Self>;

    /// Create from BigRational.
    fn from_big_rational(value: &BigRational) -> Option<Self> {
        use num::Zero;
        if value.denom().is_zero() {
            return None;
        }
        Self::from_big_int(&(value.numer() / value.denom()))
    }
}

impl U256Ext for AlloyU256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        (!other.is_zero()).then(|| self.div_ceil(*other))
    }

    fn checked_mul_f64(&self, factor: f64) -> Option<Self> {
        if !factor.is_finite() || factor.is_sign_negative() {
            return None;
        }

        // Special case: multiplication by 1.0 is identity
        // This avoids intermediate overflow when multiplying large values
        if factor == 1.0 {
            return Some(*self);
        }

        // Scale factor to preserve precision: factor * 10^18
        const SCALE_F64: f64 = 1_000_000_000_000_000_000.0;
        const SCALE_U128: u128 = 1_000_000_000_000_000_000;
        let scaled_factor = factor * SCALE_F64;

        // Convert scaled factor to U256
        let scaled_factor_u256 = if scaled_factor <= u128::MAX as f64 {
            // For values that fit in u128, convert directly
            AlloyU256::from(scaled_factor as u128)
        } else {
            // For larger values, use BigUint's f64 conversion
            use num::FromPrimitive;
            let scaled_factor_big = num::BigUint::from_f64(scaled_factor)?;
            AlloyU256::try_from(&scaled_factor_big).ok()?
        };

        // Perform: (self * scaled_factor_u256) / SCALE
        let result = self.checked_mul(scaled_factor_u256)?;
        result.checked_div(AlloyU256::from(SCALE_U128))
    }

    fn to_big_int(&self) -> BigInt {
        self.into()
    }

    fn to_big_uint(&self) -> BigUint {
        self.into()
    }

    fn to_big_rational(&self) -> BigRational {
        BigRational::new(self.to_big_int(), 1.into())
    }

    fn from_big_int(input: &BigInt) -> Option<Self> {
        AlloyU256::try_from(input).ok()
    }

    fn from_big_uint(input: &BigUint) -> Option<Self> {
        AlloyU256::try_from(input).ok()
    }
}

impl U256Ext for PrimitiveU256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(1.into())?)?
            .checked_div(*other)
    }

    fn checked_mul_f64(&self, factor: f64) -> Option<Self> {
        if !factor.is_finite() || factor.is_sign_negative() {
            return None;
        }

        // Special case: multiplication by 1.0 is identity
        // This avoids intermediate overflow when multiplying large values
        if factor == 1.0 {
            return Some(*self);
        }

        // Scale factor to preserve precision: factor * 10^18
        const SCALE_F64: f64 = 1_000_000_000_000_000_000.0;
        const SCALE_U128: u128 = 1_000_000_000_000_000_000;
        let scaled_factor = factor * SCALE_F64;

        // Convert scaled factor to U256
        let scaled_factor_u256 = if scaled_factor <= u128::MAX as f64 {
            // For values that fit in u128, convert directly
            PrimitiveU256::from(scaled_factor as u128)
        } else {
            // For larger values, use BigUint's f64 conversion
            use num::FromPrimitive;
            let scaled_factor_big = num::BigUint::from_f64(scaled_factor)?;
            let bytes = scaled_factor_big.to_bytes_be();
            if bytes.len() > 32 {
                return None;
            }
            PrimitiveU256::from_big_endian(&bytes)
        };

        // Perform: (self * scaled_factor_u256) / SCALE
        let result = self.checked_mul(scaled_factor_u256)?;
        result.checked_div(PrimitiveU256::from(SCALE_U128))
    }

    fn to_big_int(&self) -> BigInt {
        crate::conversions::u256_to_big_int(self)
    }

    fn to_big_uint(&self) -> BigUint {
        let mut bytes = [0u8; 32];
        self.to_big_endian(&mut bytes);
        BigUint::from_bytes_be(&bytes)
    }

    fn to_big_rational(&self) -> BigRational {
        crate::conversions::u256_to_big_rational(self)
    }

    fn from_big_int(input: &BigInt) -> Option<Self> {
        if input.is_negative() {
            return None;
        }
        Self::from_big_uint(input.magnitude())
    }

    fn from_big_uint(input: &BigUint) -> Option<Self> {
        let bytes = input.to_bytes_be();
        (bytes.len() <= 32).then(|| PrimitiveU256::from_big_endian(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloy_checked_ceil_div() {
        // Exact division
        assert_eq!(
            AlloyU256::from(10u64).checked_ceil_div(&AlloyU256::from(2u64)),
            Some(AlloyU256::from(5u64))
        );

        // Ceiling needed: 10 / 3 = 3.33... -> 4
        assert_eq!(
            AlloyU256::from(10u64).checked_ceil_div(&AlloyU256::from(3u64)),
            Some(AlloyU256::from(4u64))
        );

        // Ceiling needed: 7 / 2 = 3.5 -> 4
        assert_eq!(
            AlloyU256::from(7u64).checked_ceil_div(&AlloyU256::from(2u64)),
            Some(AlloyU256::from(4u64))
        );

        // 1 / 2 = 0.5 -> 1
        assert_eq!(
            AlloyU256::from(1u64).checked_ceil_div(&AlloyU256::from(2u64)),
            Some(AlloyU256::from(1u64))
        );

        // Division by 1
        assert_eq!(
            AlloyU256::from(42u64).checked_ceil_div(&AlloyU256::from(1u64)),
            Some(AlloyU256::from(42u64))
        );

        // Zero divided by anything (non-zero)
        assert_eq!(
            AlloyU256::from(0u64).checked_ceil_div(&AlloyU256::from(5u64)),
            Some(AlloyU256::from(0u64))
        );

        // Division by zero returns None
        assert_eq!(
            AlloyU256::from(10u64).checked_ceil_div(&AlloyU256::ZERO),
            None
        );

        // Large number division
        let large = AlloyU256::from(1_000_000_000_000_000_000u64); // 1e18
        let divisor = AlloyU256::from(3u64);
        // 1e18 / 3 = 333,333,333,333,333,333.33... -> 333,333,333,333,333,334
        assert_eq!(
            large.checked_ceil_div(&divisor),
            Some(AlloyU256::from(333_333_333_333_333_334u64))
        );
    }

    #[test]
    fn test_primitive_checked_ceil_div() {
        // Exact division
        assert_eq!(
            PrimitiveU256::from(10u64).checked_ceil_div(&PrimitiveU256::from(2u64)),
            Some(PrimitiveU256::from(5u64))
        );

        // Ceiling needed: 10 / 3 = 3.33... -> 4
        assert_eq!(
            PrimitiveU256::from(10u64).checked_ceil_div(&PrimitiveU256::from(3u64)),
            Some(PrimitiveU256::from(4u64))
        );

        // Ceiling needed: 7 / 2 = 3.5 -> 4
        assert_eq!(
            PrimitiveU256::from(7u64).checked_ceil_div(&PrimitiveU256::from(2u64)),
            Some(PrimitiveU256::from(4u64))
        );

        // 1 / 2 = 0.5 -> 1
        assert_eq!(
            PrimitiveU256::from(1u64).checked_ceil_div(&PrimitiveU256::from(2u64)),
            Some(PrimitiveU256::from(1u64))
        );

        // Division by 1
        assert_eq!(
            PrimitiveU256::from(42u64).checked_ceil_div(&PrimitiveU256::from(1u64)),
            Some(PrimitiveU256::from(42u64))
        );

        // Zero divided by anything (non-zero)
        assert_eq!(
            PrimitiveU256::from(0u64).checked_ceil_div(&PrimitiveU256::from(5u64)),
            Some(PrimitiveU256::from(0u64))
        );

        // Division by zero returns None
        assert_eq!(
            PrimitiveU256::from(10u64).checked_ceil_div(&PrimitiveU256::zero()),
            None
        );

        // Large number division
        let large = PrimitiveU256::from(1_000_000_000_000_000_000u64); // 1e18
        let divisor = PrimitiveU256::from(3u64);
        // 1e18 / 3 = 333,333,333,333,333,333.33... -> 333,333,333,333,333,334
        assert_eq!(
            large.checked_ceil_div(&divisor),
            Some(PrimitiveU256::from(333_333_333_333_333_334u64))
        );
    }

    #[test]
    fn test_alloy_checked_mul_f64() {
        // Realistic enough values
        let value = AlloyU256::from(25_000_000_000_000_000_000u128); // 25 ether
        let result = value.checked_mul_f64(0.2).unwrap();
        assert_eq!(
            result,
            AlloyU256::from(5_000_000_000_000_000_000u128),
            "25 ether * 0.2 must be exact"
        );

        // Basic functionality
        assert_eq!(
            AlloyU256::from(100u64).checked_mul_f64(0.5),
            Some(AlloyU256::from(50u64))
        );

        // Multiply by 1.0
        assert_eq!(
            AlloyU256::from(12345u64).checked_mul_f64(1.0),
            Some(AlloyU256::from(12345u64))
        );

        // Multiply by 0.0
        assert_eq!(
            AlloyU256::from(12345u64).checked_mul_f64(0.0),
            Some(AlloyU256::ZERO)
        );

        // Zero multiplied by any factor
        assert_eq!(
            AlloyU256::ZERO.checked_mul_f64(123.456),
            Some(AlloyU256::ZERO)
        );

        // Negative factor returns None
        assert_eq!(AlloyU256::from(100u64).checked_mul_f64(-1.0), None);

        // NaN returns None
        assert_eq!(AlloyU256::from(100u64).checked_mul_f64(f64::NAN), None);

        // Infinity returns None
        assert_eq!(AlloyU256::from(100u64).checked_mul_f64(f64::INFINITY), None);
        assert_eq!(
            AlloyU256::from(100u64).checked_mul_f64(f64::NEG_INFINITY),
            None
        );

        // Test with exact f64 representation: 0.125 = 1/8
        let value = AlloyU256::from(1_000_000_000u64); // 1 billion
        let result = value.checked_mul_f64(0.125).unwrap();
        // 1_000_000_000 * 0.125 = 125_000_000
        assert_eq!(result, AlloyU256::from(125_000_000u64));

        // Test with 0.25 = 1/4
        let value = AlloyU256::from(8_888_888u64);
        let result = value.checked_mul_f64(0.25).unwrap();
        // 8_888_888 * 0.25 = 2_222_222
        assert_eq!(result, AlloyU256::from(2_222_222u64));

        // Test with very small exact value
        let value = AlloyU256::from(1_000_000_000_000_000_000u64); // 1e18
        let result = value.checked_mul_f64(0.00390625).unwrap(); // 1/256
        // 1e18 / 256 = 3906250000000000
        assert_eq!(result, AlloyU256::from(3_906_250_000_000_000u64));

        // Multiplying a large U256 by a large factor should overflow and return None
        let max_u256 = AlloyU256::MAX;
        assert_eq!(max_u256.checked_mul_f64(1.1), None);
    }

    #[test]
    fn test_primitive_checked_mul_f64() {
        // Realistic enough values
        let value = PrimitiveU256::from(25_000_000_000_000_000_000u128); // 25 ether
        let result = value.checked_mul_f64(0.2).unwrap();
        assert_eq!(
            result,
            PrimitiveU256::from(5_000_000_000_000_000_000u128),
            "25 ether * 0.2 must be exact"
        );

        // Multiply by fractional factor
        assert_eq!(
            PrimitiveU256::from(100u64).checked_mul_f64(0.5),
            Some(PrimitiveU256::from(50u64))
        );

        // Multiply by 1.0
        assert_eq!(
            PrimitiveU256::from(12345u64).checked_mul_f64(1.0),
            Some(PrimitiveU256::from(12345u64))
        );

        // Multiply by 0.0
        assert_eq!(
            PrimitiveU256::from(12345u64).checked_mul_f64(0.0),
            Some(PrimitiveU256::zero())
        );

        // Zero multiplied by any factor
        assert_eq!(
            PrimitiveU256::zero().checked_mul_f64(123.456),
            Some(PrimitiveU256::zero())
        );

        // Negative factor returns None
        assert_eq!(PrimitiveU256::from(100u64).checked_mul_f64(-1.0), None);

        // NaN returns None
        assert_eq!(PrimitiveU256::from(100u64).checked_mul_f64(f64::NAN), None);

        // Infinity returns None
        assert_eq!(
            PrimitiveU256::from(100u64).checked_mul_f64(f64::INFINITY),
            None
        );
        assert_eq!(
            PrimitiveU256::from(100u64).checked_mul_f64(f64::NEG_INFINITY),
            None
        );
    }
}
