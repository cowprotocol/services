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
        time::Duration,
    },
    tokio::{sync::mpsc, task::JoinHandle},
    yellowstone_grpc_client::{GeyserGrpcClient, GeyserGrpcClientError},
    yellowstone_grpc_proto::tonic::Code,
};

/// Wait between attempts to bring the stream back up.
const STREAM_RETRY: Duration = Duration::from_secs(5);

/// Wait between replay passes.
const REPLAY_INTERVAL: Duration = Duration::from_secs(60);

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

    let backfiller = Decoder::rpc_driven(&config, persistence.clone());
    let replayer = Decoder::rpc_driven(&config, persistence.clone());

    // Heals parked work in the background. A failed pass only logs: every
    // piece stays parked and the next pass retries it.
    let replay_loop = async {
        loop {
            tokio::time::sleep(REPLAY_INTERVAL).await;
            if let Err(err) = replayer.replay().await {
                tracing::warn!(?err, "replay pass failed");
            }
        }
    };

    let stream_loop = async {
        let mut resume = start_slot.map_or(Resume::Watermark, Resume::From);
        loop {
            let client = connect_yellowstone(&config.yellowstone).await;
            match Ingester::serve(
                client,
                tx.clone(),
                persistence.clone(),
                latest_chain_slot.clone(),
                settlement_program,
                solflow_program,
                resume,
            )
            .await
            {
                // The decoder hung up, the select below reports why.
                Ok(()) => break,
                // The resume slot fell out of the provider's replay window.
                // Recover the gap from RPC history: the backfill moves the
                // watermark back inside the window, retrying internally and
                // panicking rather than skipping the gap.
                Err(Error::Subscribe(err)) if slot_rejection(&err) => {
                    tracing::warn!(?err, "resume subscription rejected, backfilling");
                    backfiller.backfill().await;
                    resume = Resume::Watermark;
                    // The rejection can repeat (the watermark aged out again,
                    // or a filter error shares the status code), so pace the
                    // retry.
                    tokio::time::sleep(STREAM_RETRY).await;
                }
                Err(err) => {
                    tracing::error!(?err, "stream ended, reconnecting");
                    resume = Resume::Watermark;
                    tokio::time::sleep(STREAM_RETRY).await;
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
        () = replay_loop => unreachable!("the replay loop never returns"),
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

/// Whether a rejected subscription can mean the resume slot fell out of the
/// provider's replay window. The client carries no typed cause, only a gRPC
/// status: the geyser plugin rejects an out-of-window `from_slot` with
/// `InvalidArgument` (`OutOfRange` kept for other implementations), while
/// authentication and transport failures are never about the slot.
fn slot_rejection(err: &GeyserGrpcClientError) -> bool {
    match err {
        GeyserGrpcClientError::TonicStatus(status) => {
            matches!(status.code(), Code::InvalidArgument | Code::OutOfRange)
        }
        GeyserGrpcClientError::TransportError(_) => false,
    }
}

/// Retries the yellowstone connection until it succeeds.
async fn connect_yellowstone(config: &config::Yellowstone) -> GeyserGrpcClient {
    loop {
        match yellowstone::connect(config.endpoint.clone(), config.x_token.clone()).await {
            Ok(client) => return client,
            Err(err) => {
                tracing::error!(?err, "yellowstone connection failed");
                tokio::time::sleep(STREAM_RETRY).await;
            }
        }
    }
}
