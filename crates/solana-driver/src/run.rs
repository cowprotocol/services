//! Driver entry-point logic.

use {
    crate::infra::{Api, config, observe as infra_observe},
    clap::Parser,
    solana_client::nonblocking::rpc_client::RpcClient,
    std::{path::PathBuf, sync::Arc, time::Duration},
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

    let shutdown_token = tokio_util::sync::CancellationToken::new();
    let rpc = Arc::new(RpcClient::new_with_timeout(
        config.rpc.endpoint.to_string(),
        config.rpc.request_timeout,
    ));
    let api = Api {
        addr: config.http.bind_address,
    };
    let (listener, _addr) = api.bind().await.expect("failed to bind HTTP server");
    let serve = Api::serve(listener, rpc, shutdown_token.clone());

    futures::pin_mut!(serve);
    tokio::select! {
        result = &mut serve => panic!("serve task exited: {result:?}"),
        _ = observe::shutdown::shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_token.cancel();
            // Shutdown timeout needs to be larger than the auction deadline
            match tokio::time::timeout(Duration::from_secs(20), serve).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => panic!("API shutdown exceeded timeout"),
            }
        }
    }
}
