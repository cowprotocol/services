// <crate>/tests signals to Cargo that files inside of it are integration tests.
// Integration tests are compiled into separate binaries which is slow. To avoid
// this we create one integration test here and in this test we include all the
// tests we want to run.

// Each of the following modules contains tests.
mod api_version;
mod app_data;
mod app_data_signer;
mod autopilot_leader;
mod banned_users;
mod buffers;
mod cors;
mod cow_amm;
mod database;
mod deprecated_endpoints;
mod eth_integration;
mod eth_safe;
mod ethflow;
mod haircut;
mod hooks;
mod jit_orders;
mod limit_orders;
mod liquidity;
mod liquidity_source_notification;
mod malformed_requests;
mod order_cancellation;
mod partial_fill;
mod partially_fillable_balance;
mod partially_fillable_pool;
mod place_order_with_quote;
mod protocol_fee;
mod quote_verification;
mod quoting;
mod refunder;
mod replace_order;
mod smart_contract_orders;
mod solver_competition;
mod solver_participation_guard;
mod submission;
mod token_metadata;
mod tracking_insufficient_funds;
mod trades_v2;
mod uncovered_order;
mod univ2;
mod user_surplus;
mod vault_balances;
mod wrapper;
