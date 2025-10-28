use {
    reqwest::Url,
    shared::{arguments::TracingArguments, current_block},
    std::{net::SocketAddr, path::PathBuf},
};

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// The address to bind the driver to.
    #[clap(long, env, default_value = "0.0.0.0:11088")]
    pub addr: SocketAddr,

    /// The log filter.
    #[clap(
        long,
        env,
        default_value = "warn,driver=debug,driver::infra::solver=trace,shared=debug,solver=debug"
    )]
    pub log: String,

    /// At which log level logs should be printed to stderr instead of stdout.
    #[clap(long, env)]
    pub stderr_threshold: Option<tracing::Level>,

    #[clap(flatten)]
    pub tracing: TracingArguments,

    #[clap(flatten)]
    pub current_block: current_block::Arguments,

    /// Whether to use JSON format for the logs.
    #[clap(long, env, default_value = "false")]
    pub use_json_logs: bool,

    /// The node RPC API endpoint.
    #[clap(long, env)]
    pub ethrpc: Url,

    /// The amount of RPC calls to pack into a single RPC request.
    #[clap(long, env, default_value = "20")]
    pub ethrpc_max_batch_size: usize,

    /// The maximum number of concurrent requests to the RPC node.
    #[clap(long, env, default_value = "10")]
    pub ethrpc_max_concurrent_requests: usize,

    /// Path to the driver configuration file. This file should be in TOML
    /// format. For an example see
    /// https://github.com/cowprotocol/services/blob/main/crates/driver/example.toml.
    #[clap(long, env)]
    pub config: PathBuf,
}
