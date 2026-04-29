use {
    crate::{
        api::AppState,
        arguments::Arguments,
        config::{Configuration, NetworkConfig},
        indexer::uniswap_v3::UniswapV3Indexer,
    },
    clap::Parser,
    ethrpc::{AlloyProvider, Config as EthRpcConfig, web3},
    sqlx::{PgPool, postgres::PgPoolOptions},
    std::sync::Arc,
    tokio::task::JoinSet,
};

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    initialize_observability();
    observe::metrics::setup_registry(Some("pool_indexer".into()), None);
    let config = Configuration::from_path(&args.config).expect("failed to load configuration");
    tracing::info!("pool-indexer starting");
    run(config).await;
}

pub async fn run(config: Configuration) {
    let db = connect_db(&config).await;
    let api_state = build_api_state(&db, &config.networks);

    observe::metrics::serve_metrics(
        Arc::new(AlwaysAlive),
        config.metrics.bind_address,
        Default::default(),
        Default::default(),
    );

    let mut set = JoinSet::new();
    let api_router = crate::api::router(api_state);
    let api_addr = config.api.bind_address;
    set.spawn(async move { serve(api_router, api_addr).await });

    for network in config.networks {
        let db = db.clone();
        set.spawn(async move { run_network_indexer(db, network).await });
    }

    if let Some(result) = set.join_next().await {
        panic!("pool-indexer task exited: {result:?}");
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

fn initialize_observability() {
    let log_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    observe::tracing::init::initialize(&observe::Config::new(&log_filter, None, false, None));
    observe::panic_hook::install();
}

fn build_api_state(db: &PgPool, networks: &[NetworkConfig]) -> Arc<AppState> {
    let networks = networks
        .iter()
        .map(|network| (network.name.clone(), network.chain_id))
        .collect();

    Arc::new(AppState {
        db: db.clone(),
        networks,
    })
}

async fn run_network_indexer(db: PgPool, network: NetworkConfig) {
    tracing::info!(
        network = %network.name,
        chain_id = network.chain_id,
        factories = network.factories.len(),
        "starting network indexer",
    );

    let provider = build_provider(&network);

    // Verify the configured chain_id matches the RPC. A misconfigured
    // deployment (e.g. chain_id = 1 pointed at an Arbitrum RPC) would
    // otherwise silently index Arbitrum events into the mainnet partition
    // of the shared DB.
    use alloy::providers::Provider;
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
    // by `(chain_id, contract)`.
    let mut factory_set = JoinSet::new();
    for factory in network.factories.iter().copied() {
        let indexer = UniswapV3Indexer::new(
            provider.clone(),
            db.clone(),
            &network.indexer_config(factory.address),
        );
        factory_set.spawn(run_factory_indexer(
            db.clone(),
            provider.clone(),
            indexer,
            network.clone(),
            factory,
        ));
    }

    if let Some(result) = factory_set.join_next().await {
        panic!("pool-indexer factory task exited: {result:?}");
    }
}

async fn run_factory_indexer(
    db: PgPool,
    provider: AlloyProvider,
    indexer: UniswapV3Indexer,
    network: Arc<NetworkConfig>,
    factory: crate::config::FactoryConfig,
) {
    tracing::info!(
        network = %network.name,
        chain_id = network.chain_id,
        factory = %factory.address,
        "starting factory indexer",
    );

    bootstrap_factory(&db, &provider, &indexer, &network, &factory).await;
    indexer.run(network.poll_interval()).await;
}

/// Seed + catch-up for a fresh `(chain, factory)`. A pre-existing checkpoint
/// means this pair has already been bootstrapped (e.g. a prior run seeded
/// it), in which case we skip straight to live indexing.
async fn bootstrap_factory(
    db: &PgPool,
    provider: &AlloyProvider,
    indexer: &UniswapV3Indexer,
    network: &NetworkConfig,
    factory: &crate::config::FactoryConfig,
) {
    let checkpoint = crate::db::uniswap_v3::get_checkpoint(db, network.chain_id, &factory.address)
        .await
        .expect("failed to read checkpoint");
    if checkpoint.is_some() {
        return;
    }

    let seeded_block = if let Some(subgraph_url) = network.subgraph_url.as_ref() {
        crate::subgraph_seeder::seed(
            db,
            network.name.as_str(),
            network.chain_id,
            factory.address,
            subgraph_url,
            network.seed_block,
        )
        .await
        .expect("subgraph seeding failed")
    } else {
        crate::cold_seeder::cold_seed(
            db,
            network.name.as_str(),
            network.chain_id,
            provider.clone(),
            factory.address,
            factory.deployment_block,
            network.seed_block,
        )
        .await
        .expect("cold seeding failed")
    };
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
