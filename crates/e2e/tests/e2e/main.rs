// <crate>/tests signals to Cargo that files inside of it are integration tests.
// Integration tests are compiled into separate binaries which is slow. To avoid
// this we create one integration test here and in this test we include all the
// tests we want to run.

#[macro_use]
mod setup;
mod local_node;

// Each of the following modules contains tests.
mod app_data;
mod colocation_partial_fill;
mod colocation_univ2;
mod database;
mod eth_flow;
mod eth_integration;
mod limit_orders;
mod onchain_settlement;
mod order_cancellation;
mod partially_fillable_balance;
mod partially_fillable_observed_score;
mod partially_fillable_pool;
mod pre_interaction;
mod refunder;
mod settlement_without_onchain_liquidity;
mod smart_contract_orders;
mod tracking_insufficient_funds;
mod vault_balances;
