//! Driver entry-point logic.

use {crate::infra::observe as infra_observe, clap::Parser};

/// The Solana driver command line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Log filter for the tracing framework.
    #[arg(long, env, default_value = "info,solana_driver=debug")]
    log: String,
}

/// The driver entry-point. Parses command-line arguments and runs the driver.
pub async fn start(args: impl Iterator<Item = String>) {
    let args = Args::parse_from(args);
    run(args).await;
}

/// Runs the driver, blocking until the shutdown signal is received.
pub async fn run(args: Args) {
    infra_observe::init(observe::Config::default().with_env_filter(&args.log));

    let version = observe::version::git_version();
    tracing::info!(%version, "running solana driver");

    tracing::info!("awaiting shutdown signal");
    observe::shutdown::shutdown_signal().await;
    tracing::info!("shutting down");
}
