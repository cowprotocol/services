//! Test cases.

use crate::domain::eth;

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
pub const DEFAULT_SURPLUS_FACTOR: f64 = 1e-8;

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
pub const DEFAULT_SOLVER_FEE: f64 = 1e-16;

/// The default maximum value to be payout out to solver per solution
pub const DEFAULT_SCORE_CAP: f64 = 1e-2;

pub trait IntoWei {
    fn into_wei(self) -> eth::U256;
}

impl IntoWei for f64 {
    fn into_wei(self) -> eth::U256 {
        let wei = self * 1e18;
        eth::U256::from(wei as u64)
    }
}

impl IntoWei for u64 {
    fn into_wei(self) -> eth::U256 {
        eth::U256::from(self) * eth::U256::exp10(18)
    }
}

impl IntoWei for i32 {
    fn into_wei(self) -> eth::U256 {
        (self as u64).into_wei()
    }
}
