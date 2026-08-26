//! Indexer entry point wiring.

use {
    crate::{
        config,
        config::Config,
        indexer::{
            decoder::Decoder,
            ingester::{Error, INGEST_TO_DECODER_CAPACITY, Ingester, Resume},
        },
        persistence::Postgres,
        yellowstone,
    },
    clap::Parser,
    cow_solana_rpc::{CommitmentConfig, SolanaRPC},
    observe::metrics::{DEFAULT_METRICS_PORT, LivenessChecking, serve_metrics},
    sqlx::{PgPool, postgres::PgPoolOptions},
    std::{
        net::SocketAddr,
        path::PathBuf,
        sync::{Arc, atomic::AtomicU64},
        time::{Duration, Instant},
    },
    tokio::{sync::mpsc, task::JoinHandle},
    yellowstone_grpc_client::GeyserGrpcClient,
    yellowstone_grpc_proto::tonic::Code,
};

/// First delay before bringing the stream back up, doubled per consecutive
/// failure.
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(500);

/// Longest delay between reconnect attempts.
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Doubling delay with a cap.
struct Backoff {
    next: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self {
            next: RECONNECT_BACKOFF_INITIAL,
        }
    }

    async fn wait(&mut self) {
        tokio::time::sleep(self.next).await;
        self.next = (self.next * 2).min(RECONNECT_BACKOFF_MAX);
    }

    fn reset(&mut self) {
        self.next = RECONNECT_BACKOFF_INITIAL;
    }
}

/// The Solana indexer command line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the TOML configuration file.
    #[arg(long, env)]
    config: PathBuf,

    /// Slot to start indexing from, overriding the persisted resume point
    /// for the first subscription. Must be within the provider's replay
    /// window (~150 slots).
    #[arg(long, env)]
    start_slot: Option<u64>,
}

/// The indexer entry point. Parses command line arguments and runs until a
/// shutdown signal.
pub async fn start(args: impl Iterator<Item = String>) {
    let args = Args::parse_from(args);
    let config = config::load(&args.config).await;
    observe::panic_hook::install();
    observe::tracing::init::initialize_reentrant(&config.observe_config());
    let version = observe::version::git_version();
    tracing::info!(%version, "running solana indexer");
    run(config, args.start_slot).await
}

async fn run(config: Config, start_slot: Option<u64>) {
    // The indexer writes, so the pool always points at the write URL.
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections.get())
        .connect(config.database.write_url.as_str())
        .await
        .expect("database connection");
    let mut metrics = serve_probes(pool.clone());
    let persistence = Postgres::new(pool);
    // Confirmed commitment, matching the stream subscription.
    let rpc = SolanaRPC::new_with_timeout_and_commitment(
        &config.rpc.endpoint,
        config.rpc.request_timeout,
        CommitmentConfig::confirmed(),
    );

    let (tx, rx) = mpsc::channel(INGEST_TO_DECODER_CAPACITY);
    let settlement_program = config.chain.settlement_program_id;
    let solflow_program = config.chain.solflow_program_id;
    let mut decoder = Decoder::new(
        persistence.clone(),
        rpc,
        rx,
        settlement_program,
        solflow_program,
    );
    let mut decoder_task = tokio::spawn(async move { decoder.run().await });

    let latest_chain_slot = Arc::new(AtomicU64::default());
    let stream_loop = async {
        let mut resume = start_slot.map_or(Resume::Watermark, Resume::From);
        let mut backoff = Backoff::new();
        loop {
            let client = connect_yellowstone(&config.yellowstone).await;
            let started = Instant::now();
            let result = Ingester::serve(
                client,
                tx.clone(),
                persistence.clone(),
                latest_chain_slot.clone(),
                settlement_program,
                solflow_program,
                resume,
            )
            .await;
            // A stream that outlived the longest delay was healthy, so the
            // next outage starts the backoff over.
            if started.elapsed() > RECONNECT_BACKOFF_MAX {
                backoff.reset();
            }
            match result {
                // The decoder hung up, the select below reports why.
                Ok(()) => break,
                // The provider has discarded the requested slot. The gap
                // stays unindexed until a backfill (BE-204).
                Err(Error::Stream(status)) if status.code() == Code::OutOfRange => {
                    tracing::error!(%status, "resume slot rejected, resubscribing from the live tip");
                    resume = Resume::LiveTip;
                }
                Err(err) => {
                    tracing::error!(?err, "stream ended, reconnecting");
                    resume = Resume::Watermark;
                    backoff.wait().await;
                }
            }
        }
    };

    tokio::select! {
        // The loop only breaks when the decoder hung up, so report the
        // decoder's exit.
        _ = stream_loop => {
            let result = (&mut decoder_task).await;
            tracing::error!(?result, "decoder stopped");
        }
        result = &mut decoder_task => tracing::error!(?result, "decoder stopped"),
        result = &mut metrics => tracing::error!(?result, "metrics server stopped"),
        _ = observe::shutdown::shutdown_signal() => tracing::info!("shutdown signal received"),
    }
}

/// Serve the metrics and probe routes on the metrics port.
fn serve_probes(pool: PgPool) -> JoinHandle<()> {
    serve_metrics(
        Arc::new(Liveness { pool }),
        SocketAddr::from(([0, 0, 0, 0], DEFAULT_METRICS_PORT)),
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

/// Retries the yellowstone connection until it succeeds.
async fn connect_yellowstone(config: &config::Yellowstone) -> GeyserGrpcClient {
    let mut backoff = Backoff::new();
    loop {
        match yellowstone::connect(config.endpoint.clone(), config.x_token.clone()).await {
            Ok(client) => return client,
            Err(err) => {
                tracing::error!(?err, "yellowstone connection failed");
                backoff.wait().await;
            }
        }
    }
}
