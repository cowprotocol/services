//! Binary entry: parse args, load config, serve the `/solve` API.

use {
    crate::{api::Api, config},
    clap::Parser,
    std::{net::SocketAddr, path::PathBuf},
};

/// Command-line arguments.
#[derive(Parser, Debug)]
#[clap(name = "solana-solvers")]
pub struct Args {
    /// Path to the TOML configuration file.
    #[clap(long, env = "SOLANA_SOLVERS_CONFIG")]
    pub config: PathBuf,

    /// Socket address the HTTP API binds to.
    #[clap(long, env = "SOLANA_SOLVERS_BIND", default_value = "0.0.0.0:7900")]
    pub bind: SocketAddr,
}

/// Parse args and run the solver engine until shutdown.
pub async fn start(args: impl IntoIterator<Item = String>) {
    let args = Args::parse_from(args);
    tracing_subscriber::fmt::init();
    tracing::info!(?args, "starting solana-solvers");

    let config = config::load(&args.config).await;
    let api = Api {
        addr: args.bind,
        config,
    };
    if let Err(err) = api.serve(shutdown_signal()).await {
        tracing::error!(?err, "server error");
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
