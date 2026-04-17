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
    let config = load_configuration(&args);
    tracing::info!("pool-indexer starting");
    run(config).await;
}

pub async fn run(config: Configuration) {
    validate_networks(&config.networks);

    let db = connect_db(&config).await;
    let api_state = build_api_state(&db, &config.networks);

    let mut set = JoinSet::new();
    spawn_api_task(&mut set, api_state, config.api.bind_address);

    for network in config.networks {
        spawn_network_task(&mut set, db.clone(), network);
    }

    if let Some(result) = set.join_next().await {
        panic!("pool-indexer task exited: {result:?}");
    }
}

fn initialize_observability() {
    let log_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    observe::tracing::init::initialize(&observe::Config::new(&log_filter, None, false, None));
    observe::panic_hook::install();
}

fn load_configuration(args: &Arguments) -> Configuration {
    Configuration::from_path(&args.config).expect("failed to load configuration")
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
    tracing::info!(network = %network.name, chain_id = network.chain_id, "starting indexer");

    let provider = build_provider(&network);
    let indexer = UniswapV3Indexer::new(provider.clone(), db.clone(), &network.indexer_config());

    if let Some(subgraph_url) = network.subgraph_url.as_deref() {
        let seeded_block =
            crate::seeder::seed(&db, network.chain_id, subgraph_url, network.seed_block)
                .await
                .expect("seeding failed");
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
    }
}

async fn connect_db(config: &Configuration) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(config.database.max_connections.get())
        .connect(&config.database.url)
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
