//! Orderbook entry-point logic.

use {
    crate::infra::{Api, config, observe as infra_observe},
    clap::Parser,
    observe::metrics::{DEFAULT_METRICS_PORT, LivenessChecking, serve_metrics},
    sqlx::PgPool,
    std::{path::PathBuf, sync::Arc, time::Duration},
};

/// Fails the liveness probe when the database is unreachable.
struct Liveness {
    pool: PgPool,
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}

/// The Solana orderbook command line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the TOML configuration file.
    #[arg(long, env)]
    config: PathBuf,
}

/// The orderbook entry-point. Parses command-line arguments and runs the
/// orderbook.
pub async fn start(args: impl Iterator<Item = String>) {
    let args = Args::parse_from(args);
    run(args).await;
}

/// Runs the orderbook, blocking until the shutdown signal is received.
pub async fn run(args: Args) {
    let config = config::load(&args.config).await;

    infra_observe::init(config.observe_config());

    let version = observe::version::git_version();
    tracing::info!(%version, "running solana orderbook");
    if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") {
        tracing::info!(?config, "loaded config");
    }

    let pool = PgPool::connect(config.db_url.as_str())
        .await
        .expect("database connection");

    let mut metrics_address = config.http.bind_address;
    metrics_address.set_port(DEFAULT_METRICS_PORT);
    tracing::info!(%metrics_address, "serving metrics");
    let metrics = serve_metrics(
        Arc::new(Liveness { pool: pool.clone() }),
        metrics_address,
        Default::default(),
        Default::default(),
    );

    let shutdown_token = tokio_util::sync::CancellationToken::new();
    let api = Api {
        addr: config.http.bind_address,
        pool,
    };
    let (listener, _addr) = api.bind().await.expect("failed to bind HTTP server");
    let serve = api.serve(listener, shutdown_token.clone());

    futures::pin_mut!(serve);
    tokio::select! {
        result = &mut serve => panic!("serve task exited: {result:?}"),
        result = metrics => panic!("metrics server exited: {result:?}"),
        _ = observe::shutdown::shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_token.cancel();
            match tokio::time::timeout(Duration::from_secs(20), serve).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => panic!("API shutdown exceeded timeout"),
            }
        }
    }
}
