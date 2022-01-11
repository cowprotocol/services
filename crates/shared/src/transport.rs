pub mod buffered;
pub mod dummy;
pub mod http;
pub mod instrumented;
pub mod mock;

use self::{
    http::HttpTransport,
    instrumented::{MetricTransport, TransportMetrics},
};
use crate::Web3Transport;
use reqwest::Client;
use std::{convert::TryInto as _, sync::Arc};
use web3::BatchTransport;

pub const MAX_BATCH_SIZE: usize = 100;

/// Convenience method to create our standard instrumented transport.
pub fn create_instrumented_transport<T>(
    transport: T,
    metrics: Arc<dyn TransportMetrics>,
) -> Web3Transport
where
    T: BatchTransport + Send + Sync + 'static,
    T::Out: Send + 'static,
    T::Batch: Send + 'static,
{
    Web3Transport::new(MetricTransport::new(transport, metrics))
}

/// Convenience method to create a transport from a URL.
pub fn create_test_transport(url: &str) -> Web3Transport {
    Web3Transport::new(HttpTransport::new(
        Client::new(),
        url.try_into().unwrap(),
        "".to_string(),
    ))
}

/// Like above but takes url from the environment NODE_URL.
pub fn create_env_test_transport() -> Web3Transport {
    create_test_transport(&std::env::var("NODE_URL").unwrap())
}
