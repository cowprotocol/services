pub mod alloy;
pub mod block_stream;
#[cfg(any(test, feature = "test-util"))]
pub mod mock;

use {crate::alloy::MutWallet, alloy_provider::DynProvider, reqwest::Url, std::time::Duration};

pub const MAX_BATCH_SIZE: usize = 100;

pub type AlloyProvider = DynProvider;

/// This is just a thin wrapper around providers (clients communicating
/// with the blockchain) to aid the migration from `web3` to `alloy-provider`.
/// It's able to dereference into the current provider (`web3`) but already
/// providers access to the new provider (`alloy`). That way we should be able
/// to convert each call site to use the new provider bit by bit instead of
/// having to everything at once.
#[derive(Debug, Clone)]
pub struct Web3 {
    pub provider: AlloyProvider,
    pub wallet: MutWallet,
}

impl Web3 {
    // for tests
    pub fn new_from_env() -> Self {
        let url = &std::env::var("NODE_URL").unwrap();
        Self::new_from_url(url)
    }

    pub fn new_from_url(url: &str) -> Self {
        let (alloy, wallet) = crate::alloy::provider(url, Default::default(), None);
        Self {
            provider: alloy,
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

    /// Maximum time for one HTTP request to the node, from connect to the
    /// end of the response body. A batch is one HTTP request.
    pub ethrpc_timeout: Duration,
}

/// Default for [`Config::ethrpc_timeout`].
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

impl Default for Config {
    fn default() -> Self {
        Self {
            ethrpc_max_batch_size: 20,
            ethrpc_max_concurrent_requests: 10,
            ethrpc_batch_delay: Duration::ZERO,
            ethrpc_timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// Create a Web3 instance with an optional label for observability.
pub fn web3(args: Config, url: &Url, label: Option<&str>) -> Web3 {
    let (alloy, wallet) = match (
        args.ethrpc_max_batch_size,
        args.ethrpc_max_concurrent_requests,
    ) {
        (0 | 1, 0) => alloy::unbuffered_provider(url.as_str(), args.ethrpc_timeout, label),
        _ => alloy::provider(url.as_str(), args, label),
    };

    Web3 {
        provider: alloy,
        wallet,
    }
}

#[cfg(test)]
mod test {
    use {
        crate::{Config, Web3},
        alloy_eips::BlockId,
        alloy_provider::Provider,
        std::time::{Duration, Instant},
    };

    /// A node that accepts the connection but never responds must not block
    /// the call forever, with and without batching.
    #[tokio::test]
    async fn request_times_out_on_silent_node() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url: reqwest::Url = format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        tokio::spawn(async move {
            let mut connections = Vec::new();
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                connections.push(socket);
            }
        });

        let timeout = Duration::from_millis(200);
        for (max_batch_size, max_concurrent_requests) in [(20, 10), (0, 0)] {
            let config = Config {
                ethrpc_max_batch_size: max_batch_size,
                ethrpc_max_concurrent_requests: max_concurrent_requests,
                ethrpc_batch_delay: Duration::ZERO,
                ethrpc_timeout: timeout,
            };
            let web3 = crate::web3(config, &url, None);

            let start = Instant::now();
            let result =
                tokio::time::timeout(Duration::from_secs(5), web3.provider.get_block_number())
                    .await
                    .expect("the RPC timeout did not fire");

            assert!(result.is_err());
            assert!(start.elapsed() >= timeout);
        }
    }

    #[tokio::test]
    async fn test_https() {
        let provider = Web3::new_from_url("https://rpc.mevblocker.io");
        let response = provider.provider.get_block(BlockId::latest()).await;

        if let Err(err) = response {
            // only fail the CI if we are sure the error is due to missing
            // `https` support
            if err.to_string().contains("scheme is not http") {
                eprintln!("{err:?}");
                panic!("https support is not enabled");
            } else {
                eprintln!("mevblocker error unrelated to https support: {err:?}");
            }
        }
    }
}
