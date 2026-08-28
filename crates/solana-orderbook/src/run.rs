//! Orderbook entry-point logic.

use {
    crate::infra::{Api, config, observe as infra_observe, quoter::Quoter},
    clap::Parser,
    configs::database::DatabasePoolConfig,
    observe::metrics::{DEFAULT_METRICS_PORT, LivenessChecking, serve_metrics},
    sqlx::{Executor, PgPool, postgres::PgPoolOptions},
    std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration},
    tokio::task::JoinHandle,
};

/// Connect the read pool: the replica URL wins when configured, and every
/// connection gets the read statement timeout.
async fn read_pool(database: &DatabasePoolConfig) -> PgPool {
    let url = database.read_url.as_ref().unwrap_or(&database.write_url);
    let timeout_ms = database.statement_timeout.as_millis();
    PgPoolOptions::new()
        .max_connections(database.max_connections.get())
        .after_connect(move |conn, _meta| {
            Box::pin(async move {
                conn.execute(format!("SET statement_timeout = {timeout_ms}").as_str())
                    .await?;
                Ok(())
            })
        })
        .connect(url.as_str())
        .await
        .expect("database connection")
}

/// Serve the metrics and probe routes on the API address with the metrics
/// port.
fn serve_probes(pool: PgPool, api_address: SocketAddr) -> JoinHandle<()> {
    let mut metrics_address = api_address;
    metrics_address.set_port(DEFAULT_METRICS_PORT);
    serve_metrics(
        Arc::new(Liveness { pool }),
        metrics_address,
        Default::default(),
        Default::default(),
    )
}

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
    // Secrets stay out of the log through the config types' redacting Debug
    // impls.
    tracing::info!(%version, ?config, "running solana orderbook");

    let pool = read_pool(&config.database).await;
    let metrics = serve_probes(pool.clone(), config.http.bind_address);

    let shutdown_token = tokio_util::sync::CancellationToken::new();
    let api = Api {
        addr: config.http.bind_address,
        pool,
        quoter: Quoter::new(config.quoting.driver_url.clone(), config.quoting.timeout),
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
