//! Extension trait for U256 arithmetic operations.

use {
    alloy::primitives::U256 as AlloyU256,
    num::{BigInt, BigRational, BigUint},
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

    /// Multiply U256 by f64 factor using arbitrary precision arithmetic.
    ///
    /// This method uses `BigRational` to perform the multiplication with full
    /// precision, avoiding overflow and precision loss issues. Returns `None`
    /// if:
    /// - The factor is negative
    /// - The factor cannot be converted to a rational number (NaN, infinity)
    /// - The result overflows U256
    fn checked_mul_f64(&self, factor: f64) -> Option<Self> {
        if factor.is_sign_negative() {
            return None;
        }
        let self_rational = self.to_big_rational();
        let factor_rational = BigRational::from_float(factor)?;
        let result_rational = self_rational * factor_rational;
        Self::from_big_rational(&result_rational)
    }

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
        (*value.denom() != BigInt::zero())
            .then(|| Self::from_big_int(&(value.numer() / value.denom())))
            .flatten()
    }
}

impl U256Ext for AlloyU256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        (!other.is_zero()).then(|| self.div_ceil(*other))
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
        (input.sign() != num::bigint::Sign::Minus)
            .then(|| Self::from_big_uint(input.magnitude()))
            .flatten()
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
        // Multiply by integer
        assert_eq!(
            AlloyU256::from(100u64).checked_mul_f64(2.0),
            Some(AlloyU256::from(200u64))
        );

        // Multiply by fractional factor
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
        // Multiply by integer
        assert_eq!(
            PrimitiveU256::from(100u64).checked_mul_f64(2.0),
            Some(PrimitiveU256::from(200u64))
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
