#[macro_use]
pub mod macros;

pub mod account_balances;
pub mod api;
pub mod arguments;
pub mod bad_token;
pub mod balancer_sor_api;
pub mod baseline_solver;
pub mod conversions;
pub mod current_block;
pub mod db_order_conversions;
pub mod ethcontract_error;
pub mod event_handling;
pub mod fee_subsidy;
pub mod gas_price;
pub mod gas_price_estimation;
pub mod http_client;
pub mod http_solver;
pub mod maintenance;
pub mod metrics;
pub mod network;
pub mod oneinch_api;
pub mod order_quoting;
pub mod order_validation;
pub mod paraswap_api;
pub mod price_estimation;
pub mod rate_limiter;
pub mod recent_block_cache;
pub mod remaining_amounts;
pub mod request_sharing;
pub mod signature_validator;
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

use self::{http_client::HttpClientFactory, transport::http::HttpTransport};
use ethcontract::{
    batch::CallBatch,
    dyns::{DynTransport, DynWeb3},
};
use reqwest::Url;
use std::{
    future::Future,
    time::{Duration, Instant},
};

pub type Web3Transport = DynTransport;
pub type Web3 = DynWeb3;
pub type Web3CallBatch = CallBatch<Web3Transport>;

/// Create a Web3 instance.
pub fn web3(http_factory: &HttpClientFactory, url: &Url, name: impl ToString) -> Web3 {
    let transport = Web3Transport::new(HttpTransport::new(
        http_factory.configure(|builder| builder.cookie_store(true)),
        url.clone(),
        name.to_string(),
    ));
    Web3::new(transport)
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
