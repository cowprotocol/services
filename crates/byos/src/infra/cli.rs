use {
    clap::Parser,
    shared::arguments::TracingArguments,
    std::{net::SocketAddr, path::PathBuf},
};

/// Run the BYOS (Bring Your Own Solver) engine
#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// The log filter.
    #[arg(long, env, default_value = "warn,byos=debug")]
    pub log: String,

    /// Whether to use JSON format for the logs.
    #[clap(long, env, default_value = "false")]
    pub use_json_logs: bool,

    #[clap(flatten)]
    pub tracing: TracingArguments,

    /// The socket address to bind to.
    #[arg(long, env, default_value = "127.0.0.1:7872")]
    pub addr: SocketAddr,

    /// Path to the BYOS configuration file (TOML).
    #[arg(long, env)]
    pub config: PathBuf,
}
