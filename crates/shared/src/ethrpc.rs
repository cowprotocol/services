pub mod buffered;
pub mod dummy;
pub mod extensions;
pub mod http;
pub mod mock;
pub mod multicall;

use self::{buffered::BufferedTransport, http::HttpTransport};
use crate::{arguments::duration_from_seconds, http_client::HttpClientFactory};
use ethcontract::{batch::CallBatch, dyns::DynWeb3, transport::DynTransport};
use reqwest::{Client, Url};
use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroUsize,
    time::Duration,
};

pub const MAX_BATCH_SIZE: usize = 100;

pub type Web3 = DynWeb3;
pub type Web3Transport = DynTransport;
pub type Web3CallBatch = CallBatch<Web3Transport>;

/// Command line arguments for the common Ethereum RPC connections.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// Maximum batch size for Ethereum RPC requests. Use '0' to disable batching.
    #[clap(long, env, default_value = "100")]
    pub ethrpc_max_batch_size: usize,

    /// Maximum number of concurrent requests to send to the node. Use '0' for
    /// no limit on concurrency.
    #[clap(long, env, default_value = "10")]
    pub ethrpc_max_concurrent_requests: usize,

    /// Buffering "nagle" delay to wait for additional requests before sending out
    /// an incomplete batch.
    #[clap(long, env, value_parser = duration_from_seconds, default_value = "0")]
    pub ethrpc_batch_delay: Duration,
}

impl Arguments {
    /// Returns the buffered transport configuration or `None` if batching is
    /// disabled.
    fn buffered_configuration(&self) -> Option<buffered::Configuration> {
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

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "ethrpc_max_batch_size: {}", self.ethrpc_max_batch_size)?;
        writeln!(
            f,
            "ethrpc_max_concurrent_requests: {}",
            self.ethrpc_max_concurrent_requests
        )?;

        Ok(())
    }
}

/// Create a Web3 instance.
pub fn web3(
    args: &Arguments,
    http_factory: &HttpClientFactory,
    url: &Url,
    name: impl ToString,
) -> Web3 {
    let http = HttpTransport::new(
        http_factory.configure(|builder| builder.cookie_store(true)),
        url.clone(),
        name.to_string(),
    );
    let transport = match args.buffered_configuration() {
        Some(config) => Web3Transport::new(BufferedTransport::with_config(http, config)),
        None => Web3Transport::new(http),
    };

    Web3::new(transport)
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
