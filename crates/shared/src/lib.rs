#[macro_use]
pub mod macros;

pub mod arguments;
pub mod bad_token;
pub mod balancer_sor_api;
pub mod baseline_solver;
pub mod conversions;
pub mod current_block;
pub mod ethcontract_error;
pub mod event_handling;
pub mod gas_price_estimation;
pub mod http_client;
pub mod http_solver;
pub mod maintenance;
pub mod metrics;
pub mod network;
pub mod oneinch_api;
pub mod paraswap_api;
pub mod price_estimation;
pub mod rate_limiter;
pub mod recent_block_cache;
pub mod request_sharing;
pub mod solver_utils;
pub mod sources;
pub mod subgraph;
pub mod token_info;
pub mod token_list;
pub mod trace_many;
pub mod tracing;
pub mod transport;
pub mod univ3_router_api;
pub mod web3_traits;
pub mod zeroex_api;

use ethcontract::{
    batch::CallBatch,
    dyns::{DynTransport, DynWeb3},
};
use std::{
    future::Future,
    time::{Duration, Instant},
};

pub type Web3Transport = DynTransport;
pub type Web3 = DynWeb3;
pub type Web3CallBatch = CallBatch<Web3Transport>;

/// The standard http client we use in the api and driver.
pub fn http_client(timeout: Duration) -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .timeout(timeout)
        .user_agent("cowprotocol-services/2.0.0")
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
    bytes: impl AsRef<[u8]>,
    formatter: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    formatter.write_fmt(format_args!("0x{}", hex::encode(bytes.as_ref())))
}

/// anyhow errors are not clonable natively. This is a workaround that creates a new anyhow error
/// based on formatting the error with its inner sources without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}
