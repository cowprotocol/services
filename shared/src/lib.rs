#[macro_use]
pub mod macros;

pub mod arguments;
pub mod bad_token;
pub mod baseline_solver;
pub mod conversions;
pub mod current_block;
pub mod ethcontract_error;
pub mod event_handling;
pub mod gas_price_estimation;
pub mod http_solver_api;
pub mod maintenance;
pub mod metrics;
pub mod network;
pub mod paraswap_api;
pub mod price_estimation;
pub mod recent_block_cache;
pub mod solver_utils;
pub mod sources;
pub mod subgraph;
pub mod time;
pub mod token_info;
pub mod token_list;
pub mod trace_many;
pub mod tracing;
pub mod transport;
pub mod web3_traits;
pub mod zeroex_api;

use ethcontract::dyns::{DynTransport, DynWeb3};
use std::{
    future::Future,
    time::{Duration, Instant},
};
use web3::types::Bytes;

pub type Web3Transport = DynTransport;
pub type Web3 = DynWeb3;

/// The standard http client we use in the api and driver.
pub fn http_client(timeout: Duration) -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .timeout(timeout)
        .user_agent("gp-v2-services/2.0.0")
        .build()
        .unwrap()
}

/// Run a future and callback with the time the future took. The call back can for example log the
/// time.
pub async fn measure_time<T>(future: impl Future<Output = T>, timer: impl FnOnce(Duration)) -> T {
    let start = Instant::now();
    let result = future.await;
    timer(start.elapsed());
    result
}

pub fn debug_bytes(
    bytes: &Bytes,
    formatter: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    formatter.write_fmt(format_args!("0x{}", hex::encode(&bytes.0)))
}

/// anyhow errors are not clonable natively. This is a workaround that creates a new anyhow error
/// based on formatting the error with its inner sources without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}
