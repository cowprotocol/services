// <crate>/tests signals to Cargo that files inside of it are integration tests. Integration tests
// are compiled into separate binaries which is slow. To avoid this we create one integration test
// here and in this test we include all the tests we want to run.

#[macro_use]
mod services;
mod deploy;
mod local_node;

mod limit_orders;

// Each of the following modules contains one test.
mod eth_integration;
mod onchain_settlement;
mod settlement_without_onchain_liquidity;
mod smart_contract_orders;
mod vault_balances;
