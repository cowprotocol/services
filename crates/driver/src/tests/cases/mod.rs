//! Test cases.

use {
    crate::domain::eth,
    bigdecimal::{BigDecimal, FromPrimitive, Signed, num_traits::CheckedMul},
    num::BigRational,
    number::{conversions::big_decimal_to_big_rational, u256_ext::U256Ext},
    std::str::FromStr,
};

pub mod buy_eth;
pub mod example_config;
pub mod fees;
mod flashloan_hints;
pub mod internalization;
pub mod jit_orders;
pub mod merge_settlements;
pub mod multiple_drivers;
pub mod multiple_solutions;
pub mod order_prioritization;
pub mod parallel_auctions;
pub mod protocol_fees;
pub mod quote;
pub mod settle;
pub mod solver_balance;

/// The default surplus factor. Set to a high value to ensure a positive score
/// by default. Use a surplus factor of 1 if you want to test negative scores.
pub const DEFAULT_SURPLUS_FACTOR: &str = "1e-8";

pub const DEFAULT_POOL_AMOUNT_A: u64 = 100000;
pub const DEFAULT_POOL_AMOUNT_B: u64 = 6000;
pub const DEFAULT_POOL_AMOUNT_C: u64 = 100000;
pub const DEFAULT_POOL_AMOUNT_D: u64 = 6000;

/// The order amount for orders selling token "A" for "B".
pub const AB_ORDER_AMOUNT: u64 = 50;

/// The order amount for orders selling token "A" for "D".
pub const AD_ORDER_AMOUNT: u64 = 48;

/// The order amount for orders selling token "C" for "D".
pub const CD_ORDER_AMOUNT: u64 = 40;

pub const ETH_ORDER_AMOUNT: u64 = 40;

/// The default solver fee for limit orders.
pub const DEFAULT_SOLVER_FEE: &str = "1e-16";

/// A generic wrapper struct for representing amounts in Ether using high
/// precision.
///
/// The `Ether` struct wraps numeric types in `BigRational` to facilitate
/// operations and conversions related to Ether values.
pub struct Ether(BigRational);

impl Ether {
    /// Converts the value into Wei, the smallest unit of Ethereum.
    pub fn into_wei(self) -> eth::U256 {
        BigRational::from_f64(1e18)
            .and_then(|exp| self.0.checked_mul(&exp))
            .and_then(|wei| eth::U256::from_big_rational(&wei))
            .unwrap()
    }
}

/// Extension trait for numeric types to conveniently wrap values in `Ether`.
///
/// This trait provides the `ether` method for native numeric types, allowing
/// them to be easily wrapped in an `Ether` type for further conversion into
/// Wei.
///
/// # Examples
///
/// ```
/// assert_eq(1.ether().into_wei(), U256::exp10(18))
/// assert_eq(1u64.ether().into_wei(), U256::exp10(18))
/// assert_eq("1e-18".ether().into_wei(), U256::from(1)))
/// ```
pub trait EtherExt {
    /// Converts a value into an `Ether` instance.
    fn ether(self) -> Ether
    where
        Self: Sized;
}

/// Due to the precision limitations of f64, which may lead to inaccuracies when
/// dealing with values having up to 17 decimal places, converting strings
/// directly into Ether is recommended. This approach ensures
/// precise representation and manipulation of such high-precision values.
impl EtherExt for &str {
    fn ether(self) -> Ether {
        let value = big_decimal_to_big_rational(&BigDecimal::from_str(self).unwrap());
        assert!(
            !value.is_negative(),
            "Ether supports non-negative values only"
        );
        Ether(value)
    }
}

impl EtherExt for u64 {
    fn ether(self) -> Ether {
        Ether(BigRational::from_u64(self).unwrap())
    }
}

impl EtherExt for i32 {
    fn ether(self) -> Ether {
        assert!(self >= 0, "Ether supports non-negative values only");
        Ether(BigRational::from_i32(self).unwrap())
    }
}

impl EtherExt for f64 {
    fn ether(self) -> Ether {
        assert!(self >= 0.0, "Ether supports non-negative values only");
        Ether(BigRational::from_f64(self).unwrap())
    }
}

/// Trait for approximate equality comparisons, useful for tests with rounding
/// errors.
#[cfg(test)]
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

#[cfg(test)]
impl<T> ApproxEq for T
where
    Self: Copy,
    T: Into<num::BigInt>,
{
    fn is_approx_eq(&self, other: &Self, delta: Option<f64>) -> bool {
        use num::BigInt;

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

        // Calculate relative error: |actual - expected| / |expected|
        // Ensures correct behavior with negative numbers
        let diff = (self_.clone() - other.clone()).abs();
        let calculated_delta = diff / other.abs();

        tracing::debug!("{calculated_delta} <= {expected_delta}",);

        calculated_delta <= expected_delta
    }
}

#[cfg(test)]
mod test {
    use {super::ApproxEq, crate::domain::eth};

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
        let a = eth::U256::from(999999999u64);
        let b = eth::U256::from(999999999u64);
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u256_within_threshold() {
        // Very large values with tiny relative difference
        let a = eth::U256::from(1_000_000_000_000_000_000u64);
        let b = eth::U256::from(1_000_000_000_000_000_001u64);
        assert!(a.is_approx_eq(&b, None));
        assert!(b.is_approx_eq(&a, None));
    }

    #[test]
    fn u256_exceeds_threshold() {
        let a = eth::U256::from(1_000_000u64);
        let b = eth::U256::from(1_001_000u64); // 0.1% difference
        assert!(!a.is_approx_eq(&b, None));
        assert!(!b.is_approx_eq(&a, None));
    }

    #[test]
    fn u256_zero_values() {
        let a = eth::U256::ZERO;
        let b = eth::U256::ZERO;
        assert!(a.is_approx_eq(&b, None));
    }

    #[test]
    fn u256_custom_delta() {
        // 10000 and 10500 differ by ~5%, which is within a 10% threshold
        // Note: 10000.is_approx_eq(&10500) uses |10000-10500|/10500 = 4.76%
        //       10500.is_approx_eq(&10000) uses |10500-10000|/10000 = 5%
        let a = eth::U256::from(10000u64);
        let b = eth::U256::from(10500u64);
        assert!(!a.is_approx_eq(&b, None));
        assert!(!b.is_approx_eq(&a, None));
        assert!(a.is_approx_eq(&b, Some(0.1))); // Within 10% threshold
        assert!(b.is_approx_eq(&a, Some(0.1))); // Within 10% threshold
    }

    #[test]
    fn u256_max_values() {
        let a = eth::U256::MAX;
        let b = eth::U256::MAX;
        assert!(a.is_approx_eq(&b, None));
    }
}
