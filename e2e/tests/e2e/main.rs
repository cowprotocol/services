// <crate>/tests signals to Cargo that files inside of it are integration tests. Integration tests
// are compiled into separate binaries which is slow. To avoid this we create one integration test
// here and in this test we include all the tests we want to run.

mod eth_integration;
mod ganache;
mod onchain_settlement;
#[macro_use]
mod services;
mod settlement_without_onchain_liquidity;
mod smart_contract_orders;
mod vault_balances;
