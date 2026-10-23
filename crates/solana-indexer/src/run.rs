//! Indexer entry point wiring.

use {
    crate::{
        config,
        config::Config,
        indexer::{
            decoder::Decoder,
            ingester::{INGEST_TO_DECODER_CAPACITY, Ingester, Resume},
        },
        persistence::Postgres,
        yellowstone,
    },
    clap::Parser,
    cow_solana_rpc::{CommitmentConfig, SolanaRPC},
    observe::metrics::{DEFAULT_METRICS_PORT, LivenessChecking, serve_metrics},
    sqlx::postgres::PgPoolOptions,
    std::{
        net::SocketAddr,
        path::PathBuf,
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    },
    tokio::{sync::mpsc, task::JoinHandle},
    yellowstone_grpc_client::GeyserGrpcClient,
};

/// Wait between attempts to bring the stream back up.
const STREAM_RETRY: Duration = Duration::from_secs(5);

/// Wait between replay passes.
const REPLAY_INTERVAL: Duration = Duration::from_secs(60);

/// How long after startup the Yellowstone slot filter may deliver no new
/// confirmed slot before the liveness probe fails. It delivers every chain
/// slot, about one per 400ms, with or without CoW transactions.
const MAX_SLOT_AGE: Duration = Duration::from_secs(120);

/// The Solana indexer command line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the TOML configuration file.
    #[arg(long, env)]
    config: PathBuf,

    /// Slot to start indexing from, overriding the persisted resume point
    /// for the first subscription. No backfill runs before that
    /// subscription, so slots between the persisted watermark and this one
    /// are skipped when it lies ahead and re-streamed when it lies behind.
    /// Must be within the provider's replay window (~150 slots).
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
    // `/startup` reports ready only after the first catch-up.
    let startup = Arc::new(Some(AtomicBool::new(false)));
    let latest_chain_slot = Arc::new(AtomicU64::default());
    let mut metrics = serve_probes(startup.clone(), latest_chain_slot.clone());
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
            // Scan the history between the watermark and the RPC tip before
            // subscribing: the resume slot then sits inside the provider's
            // replay window, and a stream drop never skips a slot. Nothing to
            // scan costs one slot lookup and one signature page. An operator
            // override names its own resume slot instead.
            if resume == Resume::Watermark {
                backfiller.backfill().await;
            }
            mark_started(&startup);
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
                // A rejected subscription lands here too: after the next
                // backfill its resume slot is back inside the replay window.
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
fn serve_probes(
    startup: Arc<Option<AtomicBool>>,
    latest_chain_slot: Arc<AtomicU64>,
) -> JoinHandle<()> {
    serve_metrics(
        Arc::new(Liveness {
            startup: startup.clone(),
            latest_chain_slot,
            last_seen: Mutex::new(None),
        }),
        SocketAddr::from(([0, 0, 0, 0], DEFAULT_METRICS_PORT)),
        Default::default(),
        startup,
    )
}

/// Fails the liveness probe when no new confirmed slot arrives for
/// `MAX_SLOT_AGE` after startup. The indexer stops the process on the faults
/// it cannot repair, but the stream can stall without an error, and a
/// restart resubscribes.
struct Liveness {
    startup: Arc<Option<AtomicBool>>,
    latest_chain_slot: Arc<AtomicU64>,
    /// The latest slot a probe saw, and when a probe first saw it.
    last_seen: Mutex<Option<(u64, Instant)>>,
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        // The first backfill can outlast `MAX_SLOT_AGE`, so the clock starts
        // at startup.
        let started = (*self.startup)
            .as_ref()
            .is_none_or(|flag| flag.load(Ordering::Acquire));
        if !started {
            return true;
        }
        let slot = self.latest_chain_slot.load(Ordering::Relaxed);
        let mut last_seen = self.last_seen.lock().unwrap();
        match *last_seen {
            Some((seen, at)) if seen == slot => at.elapsed() <= MAX_SLOT_AGE,
            _ => {
                *last_seen = Some((slot, Instant::now()));
                true
            }
        }
    }
}

/// Report `/startup` ready. The indexer has caught up when the first
/// backfill is complete, or at once when a `--start-slot` override skips
/// that backfill. The stream loop calls this before each subscription, so
/// only the first call has an effect.
fn mark_started(startup: &Option<AtomicBool>) {
    let Some(flag) = startup else {
        return;
    };
    if !flag.swap(true, Ordering::AcqRel) {
        tracing::info!("caught up, marking startup ready");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_started_sets_the_startup_flag_once() {
        let startup = Some(AtomicBool::new(false));
        let ready = || startup.as_ref().unwrap().load(Ordering::Acquire);

        mark_started(&startup);
        assert!(ready());
        // Each reconnect calls it again, and the flag stays set.
        mark_started(&startup);
        assert!(ready());
    }

    #[tokio::test]
    async fn liveness_fails_when_no_new_slot_arrives_after_startup() {
        let startup = Arc::new(Some(AtomicBool::new(false)));
        let latest_chain_slot = Arc::new(AtomicU64::default());
        let liveness = Liveness {
            startup: startup.clone(),
            latest_chain_slot: latest_chain_slot.clone(),
            last_seen: Mutex::new(None),
        };
        let expire = || {
            if let Some((_, at)) = liveness.last_seen.lock().unwrap().as_mut() {
                *at = Instant::now() - MAX_SLOT_AGE - Duration::from_secs(1);
            }
        };

        // The first backfill sees no slot.
        assert!(liveness.is_alive().await);
        mark_started(&startup);
        assert!(liveness.is_alive().await);
        expire();
        assert!(!liveness.is_alive().await);
        // A new confirmed slot refreshes the probe.
        latest_chain_slot.store(1, Ordering::Relaxed);
        assert!(liveness.is_alive().await);
    }
}
