use {
    crate::{
        api::AppState,
        arguments::{Arguments, Command},
        config::Configuration,
        indexer::uniswap_v3::UniswapV3Indexer,
    },
    clap::Parser,
    ethrpc::{Config as EthRpcConfig, web3},
    sqlx::postgres::PgPoolOptions,
    std::sync::Arc,
    tokio::task::JoinSet,
};

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    let log_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    observe::tracing::init::initialize(&observe::Config::new(&log_filter, None, false, None));
    observe::panic_hook::install();

    match args.command {
        Command::Run {
            config,
            subgraph_url,
            seed_block,
        } => {
            let config = Configuration::from_path(&config).expect("failed to load configuration");
            tracing::info!("pool-indexer starting");
            run(config, subgraph_url, seed_block).await;
        }
    }
}

pub async fn run(config: Configuration, subgraph_url: Option<String>, seed_block: Option<u64>) {
    let db = connect_db(&config).await;

    let w3 = web3(
        EthRpcConfig::default(),
        &config.indexer.rpc_url,
        Some("pool-indexer"),
    );

    let poll_interval = config.indexer.poll_interval();
    let chain_id = config.indexer.chain_id;
    let indexer = UniswapV3Indexer::new(w3.provider.clone(), db.clone(), &config.indexer);

    let api_state = Arc::new(AppState {
        network_name: chain_id_to_network_name(chain_id),
        db: db.clone(),
        chain_id,
    });
    let router = crate::api::router(api_state);
    let bind_address = config.api.bind_address;

    let mut set = JoinSet::new();
    set.spawn(async move { serve(router, bind_address).await });

    if let Some(url) = subgraph_url {
        set.spawn(async move {
            let seeded_block = crate::seeder::seed(&db, chain_id, &url, seed_block)
                .await
                .expect("seeding failed");
            indexer
                .catch_up(seeded_block)
                .await
                .expect("catch-up indexing failed");
            indexer.run(poll_interval).await
        });
    } else {
        set.spawn(async move { indexer.run(poll_interval).await });
    }

    if let Some(result) = set.join_next().await {
        panic!("pool-indexer task exited: {result:?}");
    }
}

fn chain_id_to_network_name(chain_id: u64) -> String {
    match chain_id {
        1 => "mainnet",
        100 => "gnosis",
        42161 => "arbitrum-one",
        8453 => "base",
        _ => "unknown",
    }
    .to_string()
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
