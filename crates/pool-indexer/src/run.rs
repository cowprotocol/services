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
        Command::Run { config } => {
            let config = Configuration::from_path(&config).expect("failed to load configuration");
            tracing::info!("pool-indexer starting");
            run(config).await;
        }
        Command::Seed {
            config,
            subgraph_url,
            block,
        } => {
            let config = Configuration::from_path(&config).expect("failed to load configuration");
            tracing::info!("pool-indexer seeding from subgraph");
            let db = connect_db(&config).await;
            crate::seeder::seed(&db, &config, &subgraph_url, block)
                .await
                .expect("seeding failed");
        }
    }
}

pub async fn run(config: Configuration) {
    let db = connect_db(&config).await;

    let w3 = web3(
        EthRpcConfig::default(),
        &config.indexer.rpc_url,
        Some("pool-indexer"),
    );

    let poll_interval = config.indexer.poll_interval();
    let indexer = UniswapV3Indexer::new(w3.provider.clone(), db.clone(), &config.indexer);

    let api_state = Arc::new(AppState {
        db,
        chain_id: config.indexer.chain_id,
    });
    let router = crate::api::router(api_state);
    let bind_address = config.api.bind_address;

    let mut set = JoinSet::new();
    set.spawn(async move { indexer.run(poll_interval).await });
    set.spawn(async move { serve(router, bind_address).await });

    if let Some(result) = set.join_next().await {
        panic!("pool-indexer task exited: {result:?}");
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
