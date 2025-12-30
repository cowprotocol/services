pub mod alloy;
pub mod block_stream;
pub mod buffered;
pub mod http;
pub mod instrumented;
#[cfg(any(test, feature = "test-util"))]
pub mod mock;

use {
    self::{buffered::BufferedTransport, http::HttpTransport},
    crate::alloy::MutWallet,
    ::alloy::providers::DynProvider,
    ethcontract::transport::DynTransport,
    reqwest::{Client, Url},
    std::{num::NonZeroUsize, time::Duration},
    web3::Transport,
};

pub const MAX_BATCH_SIZE: usize = 100;

pub type Web3Transport = DynTransport;
pub type AlloyProvider = DynProvider;

/// This is just a thin wrapper around providers (clients communicating
/// with the blockchain) to aid the migration from `web3` to `alloy-provider`.
/// It's able to dereference into the current provider (`web3`) but already
/// providers access to the new provider (`alloy`). That way we should be able
/// to convert each call site to use the new provider bit by bit instead of
/// having to everything at once.
#[derive(Debug, Clone)]
pub struct Web3<T: Transport = DynTransport> {
    pub legacy: web3::Web3<T>,
    pub alloy: AlloyProvider,
    pub wallet: MutWallet,
}

impl<T: Transport> std::ops::Deref for Web3<T> {
    type Target = web3::Web3<T>;

    fn deref(&self) -> &Self::Target {
        &self.legacy
    }
}

impl Web3<DynTransport> {
    // for tests
    pub fn new_from_env() -> Self {
        let url = &std::env::var("NODE_URL").unwrap();
        Self::new_from_url(url)
    }

    pub fn new_from_url(url: &str) -> Self {
        let legacy_transport = create_test_transport(url);
        let web3 = web3::Web3::new(legacy_transport);
        let (alloy, wallet) = crate::alloy::provider(url);
        Self {
            legacy: web3,
            alloy,
            wallet,
        }
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
    let buffered_config = args.into_buffered_configuration();
    let (legacy, alloy, wallet) = match buffered_config {
        Some(config) => {
            let legacy = Web3Transport::new(BufferedTransport::with_config(http, config));
            let (alloy, wallet) = alloy::provider(url.as_str());
            (legacy, alloy, wallet)
        }
        None => {
            let legacy = Web3Transport::new(http);
            let (alloy, wallet) = alloy::unbuffered_provider(url.as_str());
            (legacy, alloy, wallet)
        }
    };
    let instrumented = instrumented::InstrumentedTransport::new(name.to_string(), legacy);

    Web3 {
        legacy: web3::Web3::new(Web3Transport::new(instrumented)),
        alloy,
        wallet,
    }
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
