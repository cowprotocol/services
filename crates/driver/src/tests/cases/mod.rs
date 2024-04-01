//! Test cases.

use {
    crate::{domain::eth, util::conv::u256::U256Ext},
    bigdecimal::{num_traits::CheckedMul, BigDecimal, FromPrimitive, Signed},
    num::BigRational,
    number::conversions::big_decimal_to_big_rational,
    std::str::FromStr,
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
            .and_then(|wei| eth::U256::from_big_rational(&wei).ok())
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
