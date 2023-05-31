//! Test cases.

pub mod buy_eth;
pub mod example_config;
pub mod limit_order;
pub mod merge_settlements;
pub mod multiple_solutions;
pub mod negative_scores;
pub mod quote;
pub mod risk;
pub mod settle;
pub mod solve;
pub mod solver_balance;
pub mod verify_asset_flow;
pub mod verify_internalization;

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

/// With the default amounts defined above, this is the expected score range.
pub const DEFAULT_SCORE_MIN: u128 = 2989450000000000000u128;
pub const DEFAULT_SCORE_MAX: u128 = 2989470000000000000u128;
