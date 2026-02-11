use {alloy::primitives::Address, std::sync::LazyLock};

#[macro_use]
pub mod macros;

pub mod account_balances;
pub mod arguments;
pub mod bad_token;
pub mod baseline_solver;
pub mod bytes;
pub mod code_fetching;
pub mod current_block;
pub mod db_order_conversions;
pub mod encoded_settlement;
pub mod event_handling;
pub mod event_storing_helpers;
pub mod external_prices;
pub mod fee;
pub mod gas_price;
pub mod gas_price_estimation;
pub mod http_client;
pub mod http_solver;
pub mod interaction;
pub mod maintenance;
pub mod order_quoting;
pub mod order_validation;
pub mod price_estimation;
pub mod recent_block_cache;
pub mod remaining_amounts;
pub mod request_sharing;
pub mod signature_validator;
pub mod sources;
pub mod subgraph;
pub mod tenderly_api;
pub mod token_info;
pub mod token_list;
pub mod trace_many;
pub mod trade_finding;
pub mod url;
pub mod web3;
pub mod zeroex_api;

/// anyhow errors are not clonable natively. This is a workaround that creates a
/// new anyhow error based on formatting the error with its inner sources
/// without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}

// ZKSync-based chains don't use the default 0x0 account when `tx.from` is not
// specified, so we need to use a random account when sending a simulation tx.
pub static SIMULATION_ACCOUNT: LazyLock<Address> = LazyLock::new(|| Address::random());
