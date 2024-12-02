pub use ethrpc::{
    create_env_test_transport,
    create_test_transport,
    Web3,
    Web3CallBatch,
    Web3Transport,
};
use {
    crate::http_client::HttpClientFactory,
    clap::Parser,
    reqwest::Url,
    std::{
        fmt::{self, Display, Formatter},
        time::Duration,
    },
};

pub const MAX_BATCH_SIZE: usize = 100;

/// Command line arguments for the common Ethereum RPC connections.
#[derive(clap::Parser, Debug)]
#[group(skip)]
pub struct Arguments {
    /// Maximum batch size for Ethereum RPC requests. Use '0' to disable
    /// batching.
    #[clap(long, env, default_value = "100")]
    pub ethrpc_max_batch_size: usize,

    /// Maximum number of concurrent requests to send to the node. Use '0' for
    /// no limit on concurrency.
    #[clap(long, env, default_value = "10")]
    pub ethrpc_max_concurrent_requests: usize,

    /// Buffering "nagle" delay to wait for additional requests before sending
    /// out an incomplete batch.
    #[clap(long, env, value_parser = humantime::parse_duration, default_value = "0s")]
    pub ethrpc_batch_delay: Duration,
}

impl Default for Arguments {
    fn default() -> Self {
        Arguments::parse_from([""])
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            ethrpc_max_batch_size,
            ethrpc_max_concurrent_requests,
            ethrpc_batch_delay,
        } = self;

        writeln!(f, "ethrpc_max_batch_size: {}", ethrpc_max_batch_size)?;
        writeln!(
            f,
            "ethrpc_max_concurrent_requests: {}",
            ethrpc_max_concurrent_requests
        )?;
        writeln!(f, "ethrpc_batch_delay: {:?}", ethrpc_batch_delay)?;

        Ok(())
    }
}

impl Arguments {
    fn ethrpc(&self) -> ethrpc::Config {
        ethrpc::Config {
            ethrpc_max_batch_size: self.ethrpc_max_batch_size,
            ethrpc_max_concurrent_requests: self.ethrpc_max_concurrent_requests,
            ethrpc_batch_delay: self.ethrpc_batch_delay,
        }
    }
}

/// Create a Web3 instance.
pub fn web3(
    args: &Arguments,
    http_factory: &HttpClientFactory,
    url: &Url,
    name: impl ToString,
) -> Web3 {
    let http_builder = http_factory.builder();
    ethrpc::web3(args.ethrpc(), http_builder, url, name)
}
