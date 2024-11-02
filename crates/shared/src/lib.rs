#[macro_use]
pub mod macros;

pub mod account_balances;
pub mod arguments;
pub mod bad_token;
pub mod baseline_solver;
pub mod code_fetching;
pub mod code_simulation;
pub mod contracts;
pub mod conversions;
pub mod current_block;
pub mod db_order_conversions;
pub mod encoded_settlement;
pub mod ethcontract_error;
pub mod ethrpc;
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
pub mod metrics;
pub mod order_quoting;
pub mod order_validation;
pub mod price_estimation;
pub mod recent_block_cache;
pub mod remaining_amounts;
pub mod request_sharing;
pub mod signature_validator;
pub mod sources;
pub mod subgraph;
pub mod submitter_constants;
pub mod tenderly_api;
pub mod token_info;
pub mod token_list;
pub mod trace_many;
pub mod trade_finding;
pub mod url;
pub mod zeroex_api;

use std::{
    future::Future,
    time::{Duration, Instant},
};

/// Run a future and callback with the time the future took. The call back can
/// for example log the time.
pub async fn measure_time<T>(future: impl Future<Output = T>, timer: impl FnOnce(Duration)) -> T {
    let start = Instant::now();
    let result = future.await;
    timer(start.elapsed());
    result
}

pub fn debug_bytes(
    bytes: impl AsRef<[u8]>,
    formatter: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    formatter.write_fmt(format_args!("0x{}", hex::encode(bytes.as_ref())))
}

/// anyhow errors are not clonable natively. This is a workaround that creates a
/// new anyhow error based on formatting the error with its inner sources
/// without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}
