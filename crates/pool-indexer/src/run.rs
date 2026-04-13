use {
    crate::{
        api::AppState,
        arguments::Arguments,
        config::{Configuration, NetworkConfig},
        indexer::uniswap_v3::UniswapV3Indexer,
    },
    clap::Parser,
    ethrpc::{Config as EthRpcConfig, web3},
    sqlx::postgres::PgPoolOptions,
    std::{collections::HashSet, sync::Arc},
    tokio::task::JoinSet,
};

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    let log_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    observe::tracing::init::initialize(&observe::Config::new(&log_filter, None, false, None));
    observe::panic_hook::install();

    let config = Configuration::from_path(&args.config).expect("failed to load configuration");
    tracing::info!("pool-indexer starting");
    run(config).await;
}

pub async fn run(config: Configuration) {
    validate_networks(&config.networks);

    let db = connect_db(&config).await;

    let networks = config
        .networks
        .iter()
        .map(|n| (n.name.clone(), n.chain_id))
        .collect();

    let api_state = Arc::new(AppState {
        db: db.clone(),
        networks,
    });
    let router = crate::api::router(api_state);
    let bind_address = config.api.bind_address;

    let mut set = JoinSet::new();
    set.spawn(async move { serve(router, bind_address).await });

    for net in config.networks {
        let db = db.clone();
        let w3 = web3(
            EthRpcConfig::default(),
            &net.rpc_url,
            Some(&format!("pool-indexer-{}", net.name)),
        );

        let indexer_config = net.indexer_config();
        let poll_interval = net.poll_interval();
        let chain_id = net.chain_id;
        let name = net.name.clone();
        let subgraph_url = net.subgraph_url.clone();
        let seed_block = net.seed_block;

        let indexer = UniswapV3Indexer::new(w3.provider.clone(), db.clone(), &indexer_config);

        set.spawn(async move {
            tracing::info!(network = %name, chain_id, "starting indexer");
            if let Some(url) = subgraph_url {
                let seeded_block = crate::seeder::seed(&db, chain_id, &url, seed_block)
                    .await
                    .expect("seeding failed");
                indexer
                    .catch_up(seeded_block)
                    .await
                    .expect("catch-up indexing failed");
            }
            indexer.run(poll_interval).await
        });
    }

    if let Some(result) = set.join_next().await {
        panic!("pool-indexer task exited: {result:?}");
    }
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
