pub mod alloy;
pub mod block_stream;
pub mod buffered;
pub mod dummy;
pub mod extensions;
pub mod http;
pub mod instrumented;
pub mod mock;
pub mod multicall;

use {
    self::{buffered::BufferedTransport, http::HttpTransport},
    ::alloy::providers::DynProvider,
    ethcontract::{batch::CallBatch, transport::DynTransport},
    reqwest::{Client, Url},
    std::{num::NonZeroUsize, time::Duration},
    web3::Transport,
};

pub const MAX_BATCH_SIZE: usize = 100;

pub type Web3Transport = DynTransport;
pub type Web3CallBatch = CallBatch<Web3Transport>;
pub type AlloyProvider = DynProvider;

/// This is just a thin wrapper around providers (clients communicating
/// with the blockchain) to aid the migration from `web3` to `alloy-provider`.
/// It's able to dereference into the current provider (`web3`) but already
/// providers access to the new provider (`alloy`). That way we should be able
/// to convert each call site to use the new provider bit by bit instead of
/// having to everything at once.
#[derive(Debug, Clone)]
pub struct Web3<T: Transport = DynTransport> {
    pub web3: web3::Web3<T>,
    pub alloy: AlloyProvider,
}

impl<T: Transport> std::ops::Deref for Web3<T> {
    type Target = web3::Web3<T>;

    fn deref(&self) -> &Self::Target {
        &self.web3
    }
}

impl<T: Transport> Web3<T> {
    pub fn new(transport: T) -> Self {
        Self {
            web3: web3::Web3::new(transport),
            alloy: crate::alloy::provider("https://eth.llamarpc.com"),
        }
    }

    pub fn from_legacy_web3(web3: web3::Web3<T>) -> Self {
        let alloy = crate::alloy::provider("https://eth.llamarpc.com");
        Self { web3, alloy }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum batch size for Ethereum RPC requests. Use '0' to disable
    /// batching.
    pub ethrpc_max_batch_size: usize,

    /// Maximum number of concurrent requests to send to the node. Use '0' for
    /// no limit on concurrency.
    pub ethrpc_max_concurrent_requests: usize,

    /// Buffering "nagle" delay to wait for additional requests before sending
    /// out an incomplete batch.
    pub ethrpc_batch_delay: Duration,
}

impl Config {
    /// Returns the buffered transport configuration or `None` if batching is
    /// disabled.
    fn into_buffered_configuration(self) -> Option<buffered::Configuration> {
        match (
            self.ethrpc_max_batch_size,
            self.ethrpc_max_concurrent_requests,
        ) {
            (0 | 1, 0) => None,
            _ => Some(buffered::Configuration {
                max_concurrent_requests: NonZeroUsize::new(self.ethrpc_max_concurrent_requests),
                max_batch_len: self.ethrpc_max_batch_size.max(1),
                batch_delay: self.ethrpc_batch_delay,
            }),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ethrpc_max_batch_size: 20,
            ethrpc_max_concurrent_requests: 10,
            ethrpc_batch_delay: Duration::from_millis(5),
        }
    }
}

/// Create a Web3 instance.
pub fn web3(
    args: Config,
    http_factory: reqwest::ClientBuilder,
    url: &Url,
    name: impl ToString,
) -> Web3 {
    let http = http_factory.cookie_store(true).build().unwrap();
    let http = HttpTransport::new(http, url.clone(), name.to_string());
    let transport = match args.into_buffered_configuration() {
        Some(config) => Web3Transport::new(BufferedTransport::with_config(http, config)),
        None => Web3Transport::new(http),
    };
    let instrumented = instrumented::InstrumentedTransport::new(name.to_string(), transport);
    Web3::new(Web3Transport::new(instrumented))
}

/// Convenience method to create a transport from a URL.
pub fn create_test_transport(url: &str) -> Web3Transport {
    let http_transport = HttpTransport::new(
        Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap(),
        url.try_into().unwrap(),
        "test".into(),
    );
    let dyn_transport = Web3Transport::new(http_transport);
    let instrumented = instrumented::InstrumentedTransport::new("test".into(), dyn_transport);
    Web3Transport::new(instrumented)
}

/// Like above but takes url from the environment NODE_URL.
pub fn create_env_test_transport() -> Web3Transport {
    create_test_transport(&std::env::var("NODE_URL").unwrap())
}
