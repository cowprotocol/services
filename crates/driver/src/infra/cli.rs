use {
    reqwest::Url,
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
        default_value = "debug,hyper=warn,driver::infra::solver=trace"
    )]
    pub log: String,

    /// The node RPC API endpoint.
    #[clap(long, env)]
    pub ethrpc: Url,

    /// Path to the driver configuration file. This file should be in TOML
    /// format. For an example see
    /// https://github.com/cowprotocol/services/blob/main/crates/driver/example.toml.
    #[clap(long, env, conflicts_with = "config_string")]
    pub config_path: Option<PathBuf>,

    /// Like `config` but reads the configuration directly from the argument
    /// instead of a file.
    #[clap(long, env, conflicts_with = "config_path")]
    pub config_string: Option<String>,
}
