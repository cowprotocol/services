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
    sqlx::postgres::PgPoolOptions,
    std::{
        path::PathBuf,
        sync::{Arc, atomic::AtomicU64},
        time::Duration,
    },
    tokio::sync::mpsc,
    yellowstone_grpc_client::GeyserGrpcClient,
};

/// Wait between attempts to bring the stream back up.
const STREAM_RETRY: Duration = Duration::from_secs(5);

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
                // A rejected resume usually means the last indexed slot fell
                // out of the provider's replay window. Continue from the live
                // tip, the gap stays unindexed until a backfill.
                Err(Error::Subscribe(err)) if resume != Resume::LiveTip => {
                    tracing::error!(
                        ?err,
                        "resume subscription rejected, resubscribing from the live tip"
                    );
                    resume = Resume::LiveTip;
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
        _ = observe::shutdown::shutdown_signal() => tracing::info!("shutdown signal received"),
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
