//! Driver entry-point logic.

use {
    crate::infra::{config, observe as infra_observe},
    clap::Parser,
    std::path::PathBuf,
};

/// The Solana driver command line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the TOML configuration file.
    #[arg(long, env)]
    config: PathBuf,
}

/// The driver entry-point. Parses command-line arguments and runs the driver.
pub async fn start(args: impl Iterator<Item = String>) {
    let args = Args::parse_from(args);
    run(args).await;
}

/// Runs the driver, blocking until the shutdown signal is received.
pub async fn run(args: Args) {
    let config = config::load(&args.config).await;

    infra_observe::init(config.observe_config());

    let version = observe::version::git_version();
    tracing::info!(%version, "running solana driver");
    if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") {
        tracing::info!(?config, "loaded config");
    }

    tracing::info!("awaiting shutdown signal");
    observe::shutdown::shutdown_signal().await;
    tracing::info!("shutting down");
}
