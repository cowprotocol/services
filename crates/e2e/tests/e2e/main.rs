// <crate>/tests signals to Cargo that files inside of it are integration tests. Integration tests
// are compiled into separate binaries which is slow. To avoid this we create one integration test
// here and in this test we include all the tests we want to run.

#[macro_use]
mod onchain_components;
mod deploy;
mod local_node;
mod services;

// Each of the following modules contains tests.
mod eth_flow;
mod eth_integration;
mod limit_orders;
mod onchain_settlement;
mod order_cancellation;
mod refunder;
mod settlement_without_onchain_liquidity;
mod smart_contract_orders;
mod vault_balances;
