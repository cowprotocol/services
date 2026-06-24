use {
    crate::{
        api::AppState,
        arguments::Arguments,
        config::{Configuration, NetworkConfig},
        indexer::uniswap_v3::UniswapV3Indexer,
    },
    alloy_provider::Provider,
    clap::Parser,
    ethrpc::{AlloyProvider, Config as EthRpcConfig, web3},
    sqlx::{PgPool, postgres::PgPoolOptions},
    std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    tokio::task::JoinSet,
};

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    initialize_observability(&args);
    observe::metrics::setup_registry(None, None);
    let config = Configuration::from_path(&args.config).expect("failed to load configuration");
    if args.bootstrap_only {
        tracing::info!("pool-indexer bootstrap-only starting");
        bootstrap(config).await;
        tracing::info!("pool-indexer bootstrap complete, exiting");
    } else {
        tracing::info!("pool-indexer starting");
        run(config).await;
    }
}

/// Runs the bootstrap phase (seed + catch-up to the finalized head) for every
/// factory, then returns. Binds no HTTP ports — meant to run as a separate step
/// ahead of serving.
///
/// Idempotent: each factory with an existing checkpoint is skipped (see
/// [`bootstrap_factory`]), so re-running on an already-seeded DB is a fast
/// no-op that never touches the subgraph. On return, a subsequent `run` finds
/// the checkpoints present and flips `/startup` ready almost immediately.
pub async fn bootstrap(config: Configuration) {
    let db = connect_db(&config).await;
    let network = config.network;
    let provider = build_provider_checked(&network).await;
    let network = Arc::new(network);

    // Seed every factory concurrently, like the serve path.
    let mut factory_set = JoinSet::new();
    for factory in network.factories.iter().copied() {
        let indexer = UniswapV3Indexer::new(
            provider.clone(),
            db.clone(),
            &network.indexer_config(factory.address),
        );
        let db = db.clone();
        let network = network.clone();
        factory_set.spawn(async move {
            bootstrap_factory(&db, &indexer, &network, &factory).await;
        });
    }
    while let Some(result) = factory_set.join_next().await {
        result.expect("bootstrap task panicked");
    }
}

pub async fn run(config: Configuration) {
    let db = connect_db(&config).await;
    let api_state = build_api_state(&db, &config.network);

    // Flips to 200 once every factory has finished seeding + catch-up.
    let startup = Arc::new(Some(AtomicBool::new(false)));
    let barrier = Arc::new(StartupBarrier::new(
        startup.clone(),
        config.network.factories.len(),
    ));

    // Abort the metrics task when `run` exits, so tests can rebind the port.
    let _metrics = AbortOnDrop(observe::metrics::serve_metrics(
        Arc::new(AlwaysAlive),
        config.metrics.bind_address,
        Default::default(),
        startup,
    ));

    let mut set = JoinSet::new();
    let api_router = crate::api::router(api_state);
    let api_addr = config.api.bind_address;
    set.spawn(async move { serve(api_router, api_addr).await });
    set.spawn(run_network_indexer(db, config.network, barrier));

    // Both spawned tasks are infinite loops; any return is a bug, so crash
    // and let the orchestrator restart the pod.
    if let Some(result) = set.join_next().await {
        panic!("pool-indexer task exited (expected infinite loop): {result:?}");
    }
}

/// Counts down pending factory bootstraps; flips the `/startup` flag to
/// ready when the count hits zero. Latch-once for the process lifetime.
struct StartupBarrier {
    remaining: AtomicUsize,
    flag: Arc<Option<AtomicBool>>,
}

impl StartupBarrier {
    fn new(flag: Arc<Option<AtomicBool>>, total: usize) -> Self {
        Self {
            remaining: AtomicUsize::new(total),
            flag,
        }
    }

    fn factory_bootstrapped(&self) {
        if self.remaining.fetch_sub(1, Ordering::AcqRel) == 1
            && let Some(flag) = self.flag.as_ref()
        {
            flag.store(true, Ordering::Release);
            tracing::info!("all factories bootstrapped, marking startup ready");
        }
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The indexer panics on unrecoverable faults, so process-up == alive.
struct AlwaysAlive;

#[async_trait::async_trait]
impl observe::metrics::LivenessChecking for AlwaysAlive {
    async fn is_alive(&self) -> bool {
        true
    }
}

fn initialize_observability(args: &Arguments) {
    let obs_config = observe::Config::new(
        args.logging.log_filter.as_str(),
        args.logging.log_stderr_threshold,
        args.logging.use_json_logs,
        None,
    );
    observe::tracing::init::initialize(&obs_config);
    observe::panic_hook::install();
}

fn build_api_state(db: &PgPool, network: &NetworkConfig) -> Arc<AppState> {
    Arc::new(AppState {
        db: db.clone(),
        network: network.name.clone(),
    })
}

async fn run_network_indexer(db: PgPool, network: NetworkConfig, barrier: Arc<StartupBarrier>) {
    tracing::info!(
        network = %network.name,
        chain_id = network.chain_id,
        factories = network.factories.len(),
        "starting network indexer",
    );

    let provider = build_provider_checked(&network).await;
    let network = Arc::new(network);

    // One task per factory. Provider + DB pool are shared; checkpoints are
    // per-factory because they're keyed by `contract_address`.
    let mut factory_set = JoinSet::new();
    for factory in network.factories.iter().copied() {
        let indexer = UniswapV3Indexer::new(
            provider.clone(),
            db.clone(),
            &network.indexer_config(factory.address),
        );
        factory_set.spawn(run_factory_indexer(
            db.clone(),
            indexer,
            network.clone(),
            factory,
            barrier.clone(),
        ));
    }

    // The symbol/decimals backfill scans every token missing the field, so
    // one pair per process is enough (not per-factory). Spawned into the
    // same JoinSet so a panic crashes the process via the same supervisor.
    let backfill_concurrency = network.prefetch_concurrency;
    let backfill_interval = network.poll_interval();
    factory_set.spawn(crate::indexer::uniswap_v3::backfill_symbols(
        provider.clone(),
        db.clone(),
        network.name.clone(),
        backfill_concurrency,
        backfill_interval,
    ));
    factory_set.spawn(crate::indexer::uniswap_v3::backfill_decimals(
        provider.clone(),
        db.clone(),
        network.name.clone(),
        backfill_concurrency,
        backfill_interval,
    ));

    // Factory indexers + backfill are all infinite loops; any return is a
    // bug, so crash and let the orchestrator restart the pod.
    if let Some(result) = factory_set.join_next().await {
        panic!(
            "pool-indexer {}: task exited (expected infinite loop): {result:?}",
            network.name,
        );
    }
}

async fn run_factory_indexer(
    db: PgPool,
    indexer: UniswapV3Indexer,
    network: Arc<NetworkConfig>,
    factory: crate::config::FactoryConfig,
    barrier: Arc<StartupBarrier>,
) {
    tracing::info!(
        network = %network.name,
        chain_id = network.chain_id,
        factory = %factory.address,
        "starting factory indexer",
    );

    bootstrap_factory(&db, &indexer, &network, &factory).await;
    barrier.factory_bootstrapped();
    indexer.run(network.poll_interval()).await;
}

/// Seed + catch-up for a fresh factory. If a checkpoint already exists,
/// skip straight to live indexing.
async fn bootstrap_factory(
    db: &PgPool,
    indexer: &UniswapV3Indexer,
    network: &NetworkConfig,
    factory: &crate::config::FactoryConfig,
) {
    let checkpoint = crate::db::uniswap_v3::get_checkpoint(db, &factory.address)
        .await
        .expect("failed to read checkpoint");
    if let Some(block) = checkpoint {
        tracing::info!(
            chain_id = network.chain_id,
            factory = %factory.address,
            block,
            "existing checkpoint found, skipping bootstrap",
        );
        return;
    }

    let seeded_block = crate::subgraph_seeder::seed(
        db,
        network.name.as_str(),
        network.chain_id,
        factory.address,
        &network.subgraph_url,
        network.seed_block,
    )
    .await
    .expect("subgraph seeding failed");
    indexer
        .catch_up(seeded_block)
        .await
        .expect("catch-up indexing failed");
}

fn build_provider(network: &NetworkConfig) -> AlloyProvider {
    web3(
        EthRpcConfig::default(),
        &network.rpc_url,
        Some(&format!("pool-indexer-{}", network.name)),
    )
    .provider
    .clone()
}

/// Builds the RPC provider and asserts the node's chain_id matches config.
/// Catches misconfigured RPC-vs-network pairings (e.g. chain_id=1 pointed at
/// an Arbitrum node) before we index the wrong chain into the DB.
async fn build_provider_checked(network: &NetworkConfig) -> AlloyProvider {
    let provider = build_provider(network);
    let actual_chain_id = provider
        .get_chain_id()
        .await
        .expect("failed to fetch chain_id from RPC");
    assert_eq!(
        actual_chain_id, network.chain_id,
        "chain_id mismatch for network {}: config says {}, RPC reports {}",
        network.name, network.chain_id, actual_chain_id,
    );
    provider
}

async fn connect_db(config: &Configuration) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(config.database.max_connections.get())
        .connect(config.database.url.as_str())
        .await
        .expect("failed to connect to database")
}

async fn serve(router: axum::Router, addr: std::net::SocketAddr) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind TCP listener");
    tracing::info!(%addr, "serving pool-indexer API");
    axum::serve(listener, router).await.expect("server error");
}
