use {
    crate::{
        api::AppState,
        arguments::Arguments,
        config::{BalancerV2Config, Configuration, FactoryConfig, NetworkConfig},
        indexer::{
            balancer_v2::{self, BalancerV2Indexer, PoolType},
            uniswap_v3::UniswapV3Indexer,
        },
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

/// Runs the bootstrap phase (on-chain cold-seed + catch-up to the finalized
/// head) for every factory, then returns. Binds no HTTP ports; meant to run as
/// a separate step ahead of serving.
///
/// A factory already caught up to the head is a fast no-op, and an interrupted
/// seed resumes from its checkpoint (see [`bootstrap_factory`]).
/// On return every factory is indexed to the finalized head, so a later `run`
/// flips `/startup` ready promptly.
pub async fn bootstrap(config: Configuration) {
    let db = connect_db(&config).await;
    let network = config.network;
    let provider = build_provider_checked(&network).await;
    let network = Arc::new(network);

    // Seed every factory concurrently, like the serve path.
    let mut factory_set = JoinSet::new();
    if let Some(uniswap_v3) = &network.uniswap_v3 {
        for factory in uniswap_v3.factories.iter().copied() {
            let indexer = UniswapV3Indexer::new(
                provider.clone(),
                db.clone(),
                &network.indexer_config(uniswap_v3.chunk_size, factory.address),
            );
            let db = db.clone();
            let network = network.clone();
            factory_set.spawn(async move {
                bootstrap_factory(&db, &indexer, &network, &factory).await;
            });
        }
    }
    if let Some(balancer) = &network.balancer_v2 {
        for (pool_type, factory) in balancer_v2::configured_factories(balancer) {
            let indexer = BalancerV2Indexer::new(
                provider.clone(),
                db.clone(),
                balancer_indexer_config(&network, balancer, pool_type, factory),
            );
            factory_set.spawn(async move {
                indexer
                    .bootstrap()
                    .await
                    .expect("balancer bootstrap failed");
            });
        }
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
        config
            .network
            .uniswap_v3
            .as_ref()
            .map_or(0, |u| u.factories.len())
            + config
                .network
                .balancer_v2
                .as_ref()
                .map_or(0, |b| b.factory_count()),
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
        uniswap_v3_factories: network
            .uniswap_v3
            .iter()
            .flat_map(|u| &u.factories)
            .map(|f| f.address)
            .collect(),
        balancer_v2_factories: network
            .balancer_v2
            .iter()
            .flat_map(balancer_v2::configured_factories)
            .map(|(_, factory)| factory.address)
            .collect(),
    })
}

async fn run_network_indexer(db: PgPool, network: NetworkConfig, barrier: Arc<StartupBarrier>) {
    tracing::info!(
        network = %network.name,
        chain_id = network.chain_id,
        "starting network indexer",
    );

    let provider = build_provider_checked(&network).await;
    let network = Arc::new(network);

    // One task per factory (provider and DB pool shared; checkpoints are keyed
    // by factory address, so tasks never contend), plus a process-wide token
    // backfill. All tasks share one JoinSet, so any panic crashes the process.
    let mut factory_set = JoinSet::new();
    if let Some(uniswap_v3) = &network.uniswap_v3 {
        for factory in uniswap_v3.factories.iter().copied() {
            let indexer = UniswapV3Indexer::new(
                provider.clone(),
                db.clone(),
                &network.indexer_config(uniswap_v3.chunk_size, factory.address),
            );
            factory_set.spawn(run_factory_indexer(
                db.clone(),
                indexer,
                network.clone(),
                factory,
                barrier.clone(),
            ));
        }

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
    }
    if let Some(balancer) = &network.balancer_v2 {
        let poll_interval = network.poll_interval();
        for (pool_type, factory) in balancer_v2::configured_factories(balancer) {
            let indexer = BalancerV2Indexer::new(
                provider.clone(),
                db.clone(),
                balancer_indexer_config(&network, balancer, pool_type, factory),
            );
            let barrier = barrier.clone();
            factory_set.spawn(async move {
                indexer
                    .bootstrap()
                    .await
                    .expect("balancer bootstrap failed");
                barrier.factory_bootstrapped();
                indexer.run(poll_interval).await;
            });
        }
        factory_set.spawn(balancer_v2::backfill_decimals(
            provider.clone(),
            db.clone(),
            network.name.clone(),
            network.prefetch_concurrency,
            poll_interval,
        ));
    }

    // Factory indexers + backfill are all infinite loops; any return is a
    // bug, so crash and let the orchestrator restart the pod.
    if let Some(result) = factory_set.join_next().await {
        panic!(
            "pool-indexer {}: task exited (expected infinite loop): {result:?}",
            network.name,
        );
    }
}

/// Per-factory Balancer indexer config from the shared network settings.
fn balancer_indexer_config(
    network: &NetworkConfig,
    balancer: &BalancerV2Config,
    pool_type: PoolType,
    factory: FactoryConfig,
) -> balancer_v2::IndexerConfig {
    balancer_v2::IndexerConfig {
        network: network.name.clone(),
        vault: balancer.vault,
        factory: factory.address,
        pool_type,
        deploy_block: factory.deploy_block,
        chunk_size: balancer.chunk_size,
        use_latest: network.use_latest,
        fetch_concurrency: network.fetch_concurrency,
        enrich_concurrency: network.prefetch_concurrency,
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

/// Indexes a factory up to the finalized head before it's considered ready. A
/// fresh factory cold-seeds from its deploy block; one with a checkpoint (from
/// a finished seed, or one interrupted partway) resumes from there. Either way
/// it returns only once caught up, so `/startup` never flips ready on a partial
/// DB.
async fn bootstrap_factory(
    db: &PgPool,
    indexer: &UniswapV3Indexer,
    network: &NetworkConfig,
    factory: &crate::config::FactoryConfig,
) {
    let checkpoint = crate::db::get_checkpoint(db, &factory.address)
        .await
        .expect("failed to read checkpoint");
    match checkpoint {
        Some(block) => {
            tracing::info!(
                chain_id = network.chain_id,
                factory = %factory.address,
                block,
                "resuming from checkpoint",
            );
            indexer
                .catch_up_to_finalized()
                .await
                .expect("on-chain catch-up failed");
        }
        None => {
            indexer
                .catch_up(factory.deploy_block.saturating_sub(1))
                .await
                .expect("on-chain cold-seed failed");
        }
    }
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
