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
    std::{collections::HashSet, net::SocketAddr, sync::Arc},
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
    validate_networks(&config.networks);

    let db = connect_db(&config).await;
    let api_state = build_api_state(&db, &config.networks);

    observe::metrics::serve_metrics(
        Arc::new(AlwaysAlive),
        config.metrics.bind_address,
        Default::default(),
        Default::default(),
    );

    let mut set = JoinSet::new();
    spawn_api_task(&mut set, api_state, config.api.bind_address);

    for network in config.networks {
        spawn_network_task(&mut set, db.clone(), network);
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

fn spawn_api_task(set: &mut JoinSet<()>, state: Arc<AppState>, bind_address: SocketAddr) {
    let router = crate::api::router(state);
    set.spawn(async move { serve(router, bind_address).await });
}

fn spawn_network_task(set: &mut JoinSet<()>, db: PgPool, network: NetworkConfig) {
    set.spawn(async move {
        run_network_indexer(db, network).await;
    });
}

async fn run_network_indexer(db: PgPool, network: NetworkConfig) {
    tracing::info!(
        network = %network.name,
        chain_id = network.chain_id,
        factories = network.factories.len(),
        "starting network indexer",
    );

    let provider = build_provider(&network);
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

    // A checkpoint already means this (chain, factory) has been bootstrapped —
    // e.g. a prior run seeded it. Skip the seed and resume live indexing.
    let checkpoint = crate::db::uniswap_v3::get_checkpoint(&db, network.chain_id, &factory.address)
        .await
        .expect("failed to read checkpoint");

    if checkpoint.is_none() {
        let seeded_block = if let Some(subgraph_url) = network.subgraph_url.as_ref() {
            crate::subgraph_seeder::seed(
                &db,
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
                &db,
                network.name.as_str(),
                network.chain_id,
                provider,
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

    indexer.run(network.poll_interval()).await;
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

fn validate_networks(networks: &[NetworkConfig]) {
    assert!(
        !networks.is_empty(),
        "at least one [[network]] must be configured"
    );
    let mut names = HashSet::new();
    let mut chain_ids = HashSet::new();
    for n in networks {
        assert!(
            names.insert(n.name.as_str()),
            "duplicate network name: {}",
            n.name,
        );
        assert!(
            chain_ids.insert(n.chain_id),
            "duplicate chain_id: {}",
            n.chain_id,
        );
        assert!(
            !n.factories.is_empty(),
            "network {} must list at least one factory",
            n.name,
        );
        let mut seen = HashSet::new();
        for f in &n.factories {
            assert!(
                seen.insert(f.address),
                "network {}: duplicate factory {}",
                n.name,
                f.address,
            );
        }
        // A subgraph indexes one specific factory — applying one URL to many
        // factories would double-seed the wrong data. Multi-factory networks
        // must cold-seed each factory.
        assert!(
            !(n.factories.len() > 1 && n.subgraph_url.is_some()),
            "network {}: subgraph-url cannot be combined with multiple factories (omit \
             subgraph-url to cold-seed each factory)",
            n.name,
        );
    }
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
