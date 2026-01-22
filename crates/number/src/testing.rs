use {
    bigdecimal::Signed,
    num::{BigInt, BigRational, FromPrimitive},
};

/// Trait for approximate equality comparisons, useful for tests with rounding
/// errors.
pub trait ApproxEq {
    /// Checks if two values are approximately equal within a relative error
    /// threshold.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert!(100.is_approx_eq(101, Some(0.02))); // 1% diff, within 2% threshold
    /// assert!(!100.is_approx_eq(150, Some(0.02))); // 50% diff, exceeds threshold
    /// assert!(100.is_approx_eq(100, None)); // Default 1e-9 threshold
    /// ```
    fn is_approx_eq(&self, other: &Self, delta: Option<f64>) -> bool;
}

impl<T> ApproxEq for T
where
    Self: Copy,
    T: Into<BigInt>,
{
    fn is_approx_eq(&self, other: &Self, delta: Option<f64>) -> bool {
        let self_: BigInt = (*self).into();
        let self_ = BigRational::from_integer(self_);

        let other: BigInt = (*other).into();
        let other = BigRational::from_integer(other);

        // Early equality check prevents division by zero when both values are 0
        if self_ == other {
            return true;
        }

        // Default to 1e-9 (0.0000001%) relative error threshold
        let expected_delta = BigRational::from_f64(delta.unwrap_or(0.000000001))
            .expect("delta should be representable using BigRational");

        // We can't use num::Unsigned due to ruint::U256 not implementing it
        // (due to limitations on const generics)
        // Calculate relative error: |actual - expected| / |expected|
        // Ensures correct behavior with negative numbers
        let diff = (self_.clone() - other.clone()).abs();
        let calculated_delta = diff / other.abs();

        calculated_delta <= expected_delta
    }
}

#[cfg(test)]
mod test {
    use {super::*, alloy::primitives::U256};

    #[test]
    fn u64_identical_values() {
        let a: u64 = 100;
        let b: u64 = 100;
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u64_within_threshold() {
        // 1000000000 and 1000000001 differ by 1e-9 (exactly at threshold)
        let a: u64 = 1_000_000_000;
        let b: u64 = 1_000_000_001;
        assert!(a.is_approx_eq(&b, None));
        assert!(b.is_approx_eq(&a, None));
    }

    #[test]
    fn u64_exceeds_threshold() {
        // 100 and 101 differ by 1% which exceeds the 1e-9 threshold
        let a: u64 = 100;
        let b: u64 = 101;
        assert!(!a.is_approx_eq(&b, None));
        assert!(!b.is_approx_eq(&a, None));
    }

    #[test]
    fn u64_zero_values() {
        let a: u64 = 0;
        let b: u64 = 0;
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u64_custom_delta() {
        // 100 and 105 differ by ~5%, which is within a 10% threshold
        // Note: 100.is_approx_eq(&105) uses |100-105|/105 = 4.76%
        //       105.is_approx_eq(&100) uses |105-100|/100 = 5%
        let a: u64 = 100;
        let b: u64 = 105;
        assert!(!a.is_approx_eq(&b, None)); // Not within default threshold
        assert!(!b.is_approx_eq(&a, None)); // Not within default threshold
        assert!(a.is_approx_eq(&b, Some(0.1))); // Within 10% threshold
        assert!(b.is_approx_eq(&a, Some(0.1))); // Within 10% threshold
    }

    #[test]
    fn u128_identical_values() {
        let a: u128 = 123456789012345678;
        let b: u128 = 123456789012345678;
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u128_within_threshold() {
        // Large values with tiny relative difference
        let a: u128 = 1_000_000_000_000_000_000;
        let b: u128 = 1_000_000_000_000_000_001;
        assert!(a.is_approx_eq(&b, None));
        assert!(b.is_approx_eq(&a, None));
    }

    #[test]
    fn u128_exceeds_threshold() {
        let a: u128 = 1_000_000;
        let b: u128 = 1_001_000; // 0.1% difference, exceeds threshold
        assert!(!a.is_approx_eq(&b, None));
        assert!(!b.is_approx_eq(&a, None));
    }

    #[test]
    fn u128_zero_values() {
        let a: u128 = 0;
        let b: u128 = 0;
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u128_custom_delta() {
        // 1000 and 1050 differ by ~5%, which is within a 10% threshold
        // Note: 1000.is_approx_eq(&1050) uses |1000-1050|/1050 = 4.76%
        //       1050.is_approx_eq(&1000) uses |1050-1000|/1000 = 5%
        let a: u128 = 1000;
        let b: u128 = 1050;
        assert!(!a.is_approx_eq(&b, None));
        assert!(!b.is_approx_eq(&a, None));
        assert!(a.is_approx_eq(&b, Some(0.1))); // Within 10% threshold
        assert!(b.is_approx_eq(&a, Some(0.1))); // Within 10% threshold
    }

    #[test]
    fn u256_identical_values() {
        let a = U256::from(999999999u64);
        let b = U256::from(999999999u64);
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u256_within_threshold() {
        // Very large values with tiny relative difference
        let a = U256::from(1_000_000_000_000_000_000u64);
        let b = U256::from(1_000_000_000_000_000_001u64);
        assert!(a.is_approx_eq(&b, None));
        assert!(b.is_approx_eq(&a, None));
    }

    #[test]
    fn u256_exceeds_threshold() {
        let a = U256::from(1_000_000u64);
        let b = U256::from(1_001_000u64); // 0.1% difference
        assert!(!a.is_approx_eq(&b, None));
        assert!(!b.is_approx_eq(&a, None));
    }

    #[test]
    fn u256_zero_values() {
        let a = U256::ZERO;
        let b = U256::ZERO;
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u256_custom_delta() {
        // 10000 and 10500 differ by ~5%, which is within a 10% threshold
        // Note: 10000.is_approx_eq(&10500) uses |10000-10500|/10500 = 4.76%
        //       10500.is_approx_eq(&10000) uses |10500-10000|/10000 = 5%
        let a = U256::from(10000u64);
        let b = U256::from(10500u64);
        assert!(!a.is_approx_eq(&b, None));
        assert!(!b.is_approx_eq(&a, None));
        assert!(a.is_approx_eq(&b, Some(0.1))); // Within 10% threshold
        assert!(b.is_approx_eq(&a, Some(0.1))); // Within 10% threshold
    }

    #[test]
    fn u256_max_values() {
        let a = U256::MAX;
        let b = U256::MAX;
        assert!(a.is_approx_eq(&b, None));
    }
}
