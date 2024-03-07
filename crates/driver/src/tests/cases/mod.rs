//! Test cases.

use {
    crate::{domain::eth, util::conv::u256::U256Ext},
    bigdecimal::{num_bigint::ToBigInt, BigDecimal, Signed},
    std::{ops::Mul, str::FromStr},
};

pub mod buy_eth;
pub mod example_config;
pub mod fees;
pub mod internalization;
pub mod merge_settlements;
pub mod multiple_drivers;
pub mod multiple_solutions;
pub mod negative_scores;
pub mod order_prioritization;
pub mod protocol_fees;
pub mod quote;
pub mod score_competition;
pub mod settle;
pub mod solver_balance;

#[allow(dead_code)]
/// Example solver name.
const SOLVER_NAME: &str = "test1";

/// The default surplus factor. Set to a high value to ensure a positive score
/// by default. Use a surplus factor of 1 if you want to test negative scores.
pub const DEFAULT_SURPLUS_FACTOR: &str = "1e-8";

pub const DEFAULT_POOL_AMOUNT_A: u64 = 100000;
pub const DEFAULT_POOL_AMOUNT_B: u64 = 6000;
pub const DEFAULT_POOL_AMOUNT_C: u64 = 100000;
pub const DEFAULT_POOL_AMOUNT_D: u64 = 6000;

/// The order amount for orders selling token "A" for "B".
pub const AB_ORDER_AMOUNT: u64 = 50;

/// The order amount for orders selling token "C" for "D".
pub const CD_ORDER_AMOUNT: u64 = 40;

pub const ETH_ORDER_AMOUNT: u64 = 40;

/// With the default amounts defined above, this is the expected score range for
/// both buy and sell orders.
pub const DEFAULT_SCORE_MIN: u64 = 2;
pub const DEFAULT_SCORE_MAX: u64 = 500000000000;

/// The default solver fee for limit orders.
pub const DEFAULT_SOLVER_FEE: &str = "1e-16";

/// The default maximum value to be payout out to solver per solution
pub const DEFAULT_SCORE_CAP: &str = "1e-2";

/// A generic wrapper struct for representing amounts in Ether.
///
/// The `Ether` struct is designed to wrap numeric types, facilitating
/// operations and conversions related to Ether values.
pub struct Ether<T>(T);

/// A trait for converting values into Wei, the smallest denomination of Ether.
///
/// This trait defines a method `into_wei` that converts the wrapped Ether value
/// into Wei. It is implemented for the `Ether` struct with different numeric
/// types.
///
/// # Examples
///
/// ```
/// let wei = 1.ether().into_wei()
/// ```
pub trait IntoWei {
    fn into_wei(self) -> eth::U256;
}

impl IntoWei for Ether<BigDecimal> {
    fn into_wei(self) -> eth::U256 {
        assert!(
            !self.0.is_negative(),
            "IntoWei supports non-negative values only"
        );
        let exp = BigDecimal::from_str("1e18").unwrap();
        let wei = self.0.mul(exp).to_bigint().unwrap();
        eth::U256::from_big_int(&wei).unwrap()
    }
}

impl IntoWei for Ether<u64> {
    fn into_wei(self) -> eth::U256 {
        eth::U256::from(self.0) * eth::U256::exp10(18)
    }
}

impl IntoWei for Ether<i32> {
    fn into_wei(self) -> eth::U256 {
        assert!(self >= 0, "IntoWei supports non-negative values only");
        Ether(self.0 as u64).into_wei()
    }
}

/// Extension trait for numeric types to conveniently wrap values in `Ether`.
///
/// This trait provides the `ether` method for native numeric types, allowing
/// them to be easily wrapped in an `Ether` type for further conversion into Wei
/// using the `IntoWei` trait.
///
/// # Examples
///
/// ```
/// let ether = 1.0f64.ether(); // Wraps 1.0 (f64) in an Ether type
/// ```
pub trait EtherExt {
    type Output;

    fn ether(self) -> Ether<Self::Output>
    where
        Self: Sized;
}

/// Due to the precision limitations of f64, which may lead to inaccuracies when
/// dealing with values having up to 17 decimal places, converting strings
/// directly into Ether<BigDecimal> is recommended. This approach ensures
/// precise representation and manipulation of such high-precision values.
impl EtherExt for &str {
    type Output = BigDecimal;

    fn ether(self) -> Ether<BigDecimal> {
        let value = BigDecimal::from_str(self).unwrap();
        assert!(
            !value.is_negative(),
            "Ether supports non-negative values only"
        );
        Ether(value)
    }
}

impl EtherExt for u64 {
    type Output = u64;

    fn ether(self) -> Ether<u64> {
        Ether(self)
    }
}

impl EtherExt for i32 {
    type Output = i32;

    fn ether(self) -> Ether<i32> {
        assert!(self >= 0, "Ether supports non-negative values only");
        Ether(self)
    }
}
