pub use ethrpc::Web3;
use {
    std::{
        fmt::{self, Display, Formatter},
        time::Duration,
    },
    url::Url,
};

pub const MAX_BATCH_SIZE: usize = 100;

/// Command line arguments for the common Ethereum RPC connections.
#[derive(clap::Parser)]
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

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            ethrpc_max_batch_size,
            ethrpc_max_concurrent_requests,
            ethrpc_batch_delay,
        } = self;

        writeln!(f, "ethrpc_max_batch_size: {ethrpc_max_batch_size}")?;
        writeln!(
            f,
            "ethrpc_max_concurrent_requests: {ethrpc_max_concurrent_requests}"
        )?;
        writeln!(f, "ethrpc_batch_delay: {ethrpc_batch_delay:?}")?;

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

/// Create a Web3 instance with a label for observability.
pub fn web3(args: &Arguments, url: &Url, name: impl ToString) -> Web3 {
    let label = name.to_string();
    ethrpc::web3(args.ethrpc(), url, Some(&label))
}

/// Builds a web3 client that sends requests one by one.
pub fn unbuffered_web3(ethrpc: &Url, name: impl ToString) -> Web3 {
    web3(&Arguments {
        ethrpc_max_batch_size: 0,
        ethrpc_max_concurrent_requests: 0,
        ethrpc_batch_delay: Default::default()
    }, ethrpc, name)
}