//! Autopilot entry point wiring.

use {
    crate::{
        domain::arbitrator::SolanaArbitrator,
        infra::{
            competition::DriverCompetition,
            config::{self, Config},
            db,
            driver::Driver,
            executor::DriverExecutor,
            listen::ListenSession,
            observation::SettlementWindows,
            observer::CompetitionObserver,
            provider::DbAuctionProvider,
            trigger::SlotTrigger,
        },
        run_loop::AuctionLoop,
    },
    chain_types::solana::Pubkey,
    clap::Parser,
    cow_solana_rpc::{CommitmentConfig, SolanaRPC},
    observe::metrics::LivenessChecking,
    sqlx::postgres::PgPoolOptions,
    std::{
        net::SocketAddr,
        path::PathBuf,
        sync::{Arc, RwLock},
        time::{Duration, Instant},
    },
};

/// Fails the liveness probe when the auction loop stops completing cycles.
struct Liveness {
    max_auction_age: Duration,
    last_cycle: RwLock<Instant>,
}

impl Liveness {
    fn new(max_auction_age: Duration) -> Self {
        Self {
            max_auction_age,
            last_cycle: RwLock::new(Instant::now()),
        }
    }

    fn record_cycle(&self) {
        *self.last_cycle.write().unwrap() = Instant::now();
    }
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        self.last_cycle.read().unwrap().elapsed() <= self.max_auction_age
    }
}

/// The Solana autopilot command line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the TOML configuration file.
    #[arg(long, env)]
    config: PathBuf,
}

/// The autopilot entry point. Parses command line arguments and runs until a
/// shutdown signal.
pub async fn start(args: impl Iterator<Item = String>) {
    let args = Args::parse_from(args);
    let config = config::load(&args.config).await;
    observe::panic_hook::install();
    observe::tracing::init::initialize_reentrant(&config.observe_config());
    let version = observe::version::git_version();
    tracing::info!(%version, "running solana autopilot");
    run(config).await
}

async fn run(config: Config) {
    // The autopilot writes settlement executions, so the pool always points
    // at the write URL.
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections.get())
        .connect(config.database.write_url.as_str())
        .await
        .expect("database connection");

    let windows = SettlementWindows::new(pool.clone());
    let listen = ListenSession::spawn(
        pool.clone(),
        db::SETTLEMENT_FINALIZED_CHANNEL,
        windows.clone(),
    );

    let rpc = SolanaRPC::new_with_timeout_and_commitment(
        &config.rpc.endpoint,
        config.rpc.request_timeout,
        CommitmentConfig::confirmed(),
    );

    let drivers: Vec<Arc<Driver>> = config
        .drivers
        .iter()
        .map(|driver| Arc::new(Driver::new(driver.name.clone(), &driver.url)))
        .collect();

    let auction_loop = AuctionLoop::new(
        Box::new(SlotTrigger::new(rpc, config.min_auction_interval)),
        Box::new(DbAuctionProvider::new(pool.clone())),
        Box::new(DriverCompetition::new(
            drivers.clone(),
            config.competition.solve_deadline,
        )),
        Box::new(SolanaArbitrator::new(
            config.competition.max_winners.get(),
            Pubkey(config.contracts.wrapped_native_mint.to_bytes()),
        )),
        Box::new(DriverExecutor::new(drivers, windows.clone())),
        Box::new(CompetitionObserver::new(pool, windows)),
        config.competition.submission_deadline_slots.get(),
    );
    let liveness = Arc::new(Liveness::new(config.max_auction_age));
    let metrics = observe::metrics::serve_metrics(
        liveness.clone(),
        SocketAddr::from(([0, 0, 0, 0], config.metrics_port)),
        Default::default(),
        Default::default(),
    );
    // TODO: recorded even when the cycle failed, so a dead database stays
    // live as long as the trigger fires. Distinguishing failed cycles is
    // BE-200.
    let cycles = auction_loop.run_forever(move || liveness.record_cycle());

    // The metrics server and the listen session never end on their own, so an
    // end means the task panicked.
    tokio::select! {
        _ = cycles => unreachable!("the auction loop never returns"),
        _ = metrics => panic!("metrics server stopped"),
        _ = listen => panic!("settlement listen session stopped"),
        () = observe::shutdown::shutdown_signal() => tracing::info!("shutting down"),
    }
}
