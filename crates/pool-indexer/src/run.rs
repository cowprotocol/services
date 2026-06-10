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
    tracing::info!("pool-indexer starting");
    run(config).await;
}

pub async fn run(config: Configuration) {
    let db = connect_db(&config).await;
    let api_state = build_api_state(&db, &config.network);

    // Startup probe: 200 once every configured factory has completed its
    // initial seed + catch-up. Until then, `/pools` returns 503 anyway
    // (no checkpoint yet).
    let startup = Arc::new(Some(AtomicBool::new(false)));
    let barrier = Arc::new(StartupBarrier::new(
        startup.clone(),
        config.network.factories.len(),
    ));

    observe::metrics::serve_metrics(
        Arc::new(AlwaysAlive),
        config.metrics.bind_address,
        Default::default(),
        startup,
    );

    let mut set = JoinSet::new();
    let api_router = crate::api::router(api_state);
    let api_addr = config.api.bind_address;
    set.spawn(async move { serve(api_router, api_addr).await });
    set.spawn(run_network_indexer(db, config.network, barrier));

    // All spawned tasks (API server + network indexer) are infinite loops;
    // any completion is a bug, so we crash the process and let the
    // orchestrator restart the pod.
    if let Some(result) = set.join_next().await {
        panic!("pool-indexer task exited (expected infinite loop): {result:?}");
    }
}

/// Tracks pending factory bootstraps so the `/startup` probe flips to 200
/// only once every configured factory has finished seeding + catching up.
/// Latched once — bootstraps don't repeat over the process lifetime.
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

/// Minimal liveness that always reports alive. The indexer panics on
/// unrecoverable faults, so if the process is up it's alive.
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

    let provider = build_provider(&network);

    // Verify the configured chain_id matches the RPC. A misconfigured
    // deployment (e.g. chain_id = 1 pointed at an Arbitrum RPC) would
    // otherwise silently index Arbitrum events into a DB whose schema
    // assumes a different network.
    let actual_chain_id = provider
        .get_chain_id()
        .await
        .expect("failed to fetch chain_id from RPC");
    assert_eq!(
        actual_chain_id, network.chain_id,
        "chain_id mismatch for network {}: config says {}, RPC reports {}",
        network.name, network.chain_id, actual_chain_id,
    );

    let network = Arc::new(network);

    // One indexer task per factory, sharing the same provider and DB pool.
    // Seeder + catch-up are per-factory because their checkpoints are keyed
    // by `contract_address`.
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

    // Symbol/decimals backfill iterates *all* tokens missing the field,
    // regardless of which factory owns the pool that referenced them — so a
    // single pair per process is enough. Spawned into the same `factory_set`
    // so a panic in either task surfaces through the same supervisor as the
    // live indexers and crashes the process — kubernetes restarts the pod,
    // and the existing `restarts` metric pages on-call.
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

    // All spawned tasks (factory indexers + symbol/decimals backfill) are
    // infinite loops; any completion is a bug, so we crash the process and
    // let the orchestrator restart the pod.
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

/// Seed + catch-up for a fresh factory. A pre-existing checkpoint means
/// this factory has already been bootstrapped (e.g. a prior run seeded
/// it), in which case we skip straight to live indexing.
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
