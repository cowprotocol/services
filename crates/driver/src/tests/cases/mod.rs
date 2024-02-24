//! Test cases.

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
pub const DEFAULT_SURPLUS_FACTOR: u64 = 10000000000u64;

pub const DEFAULT_POOL_AMOUNT_A: u128 = 100000000000000000000000u128;
pub const DEFAULT_POOL_AMOUNT_B: u128 = 6000000000000000000000u128;
pub const DEFAULT_POOL_AMOUNT_C: u128 = 100000000000000000000000u128;
pub const DEFAULT_POOL_AMOUNT_D: u128 = 6000000000000000000000u128;

/// The order amount for orders selling token "A" for "B".
pub const AB_ORDER_AMOUNT: u128 = 50000000000000000000u128;

/// The order amount for orders selling token "C" for "D".
pub const CD_ORDER_AMOUNT: u128 = 40000000000000000000u128;

pub const ETH_ORDER_AMOUNT: u128 = 40000000000000000000u128;

/// With the default amounts defined above, this is the expected score range for
/// both buy and sell orders.
pub const DEFAULT_SCORE_MIN: u128 = 2000000000000000000u128;
pub const DEFAULT_SCORE_MAX: u128 = 500000000000000000000000000000u128;

/// The default solver fee for limit orders.
pub const DEFAULT_SOLVER_FEE: u128 = 100u128;

/// The default maximum value to be payout out to solver per solution
pub const DEFAULT_SCORE_CAP: u128 = 10000000000000000u128;
