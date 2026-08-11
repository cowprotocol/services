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
    let config = config::file::load(&args.config).await;

    infra_observe::init(config.observe_config());

    let version = observe::version::git_version();
    tracing::info!(%version, "running solana driver");
    tracing::info!(?config, "loaded config");

    tracing::info!("awaiting shutdown signal");
    shutdown_signal().await;
    tracing::info!("shutting down");
}

/// Wait for the shutdown signal.
#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept signals for graceful shutdown. Kubernetes sends sigterm, Ctrl-C
    // sends sigint.
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    let sigint = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("failed to install SIGINT handler")
            .recv()
            .await;
    };
    futures::pin_mut!(sigint);
    futures::pin_mut!(sigterm);
    futures::future::select(sigterm, sigint).await;
}

/// Wait for the shutdown signal.
#[cfg(windows)]
async fn shutdown_signal() {
    // No support for signal handling on Windows.
    std::future::pending().await
}
