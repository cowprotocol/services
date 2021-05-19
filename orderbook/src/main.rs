use contracts::{GPv2Settlement, WETH9};
use model::{order::OrderUid, DomainSeparator};
use orderbook::{
    account_balances::Web3BalanceFetcher,
    database::{Database, OrderFilter},
    event_updater::EventUpdater,
    fee::EthAwareMinFeeCalculator,
    metrics::Metrics,
    orderbook::Orderbook,
    serve_task, verify_deployed_contract_constants,
};
use prometheus::Registry;
use shared::{
    bad_token::list_based::{ListBasedDetector, UnknownTokenStrategy},
    current_block::current_block_stream,
    maintenance::ServiceMaintenance,
    pool_aggregating::{self, PoolAggregator},
    pool_fetching::CachedPoolFetcher,
    price_estimate::BaselinePriceEstimator,
    transport::LoggingTransport,
};
use std::{
    collections::HashSet, iter::FromIterator as _, net::SocketAddr, sync::Arc, time::Duration,
};
use structopt::StructOpt;
use tokio::task;
use url::Url;

#[derive(Debug, StructOpt)]
struct Arguments {
    #[structopt(flatten)]
    shared: shared::arguments::Arguments,

    #[structopt(long, env = "BIND_ADDRESS", default_value = "0.0.0.0:8080")]
    bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[structopt(long, env = "DB_URL", default_value = "postgresql://")]
    db_url: Url,

    /// Skip syncing past events (useful for local deployments)
    #[structopt(long)]
    skip_event_sync: bool,

    /// The minimum amount of time an order has to be valid for.
    #[structopt(
        long,
        env = "MIN_ORDER_VALIDITY_PERIOD",
        default_value = "60",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_order_validity_period: Duration,

    /// Don't use the trace_callMany api that only some nodes support to check whether a token
    /// should be denied.
    /// Note that if a node does not support the api we still use the less accurate call api.
    #[structopt(long, env = "SKIP_TRACE_API")]
    skip_trace_api: bool,
}

pub async fn database_metrics(metrics: Arc<Metrics>, database: Database) -> ! {
    loop {
        match database.count_rows_in_tables().await {
            Ok(counts) => {
                for (table, count) in counts {
                    metrics.set_table_row_count(table, count);
                }
            }
            Err(err) => tracing::error!(?err, "failed to update db metrics"),
        };
        tokio::time::delay_for(Duration::from_secs(10)).await;
    }
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    shared::tracing::initialize(args.shared.log_filter.as_str());
    tracing::info!("running order book with {:#?}", args);

    let transport = LoggingTransport::new(
        web3::transports::Http::new(args.shared.node_url.as_str())
            .expect("transport creation failed"),
    );
    let web3 = web3::Web3::new(transport);
    let settlement_contract = GPv2Settlement::deployed(&web3)
        .await
        .expect("Couldn't load deployed settlement");
    let gp_allowance = settlement_contract
        .allowance_manager()
        .call()
        .await
        .expect("Couldn't get allowance manager address");
    let native_token = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();

    verify_deployed_contract_constants(&settlement_contract, chain_id)
        .await
        .expect("Deployed contract constants don't match the ones in this binary");
    let domain_separator =
        DomainSeparator::get_domain_separator(chain_id, settlement_contract.address());
    let database = Database::new(args.db_url.as_str()).expect("failed to create database");

    let sync_start = if args.skip_event_sync {
        web3.eth()
            .block_number()
            .await
            .map(|block| block.as_u64())
            .ok()
    } else {
        None
    };

    let event_updater =
        EventUpdater::new(settlement_contract.clone(), database.clone(), sync_start);
    let balance_fetcher = Web3BalanceFetcher::new(
        web3.clone(),
        gp_allowance,
        settlement_contract.address(),
        !args.skip_trace_api,
    );

    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            &reqwest::Client::new(),
            &web3,
            args.shared.gas_estimators.as_slice(),
        )
        .await
        .expect("failed to create gas price estimator"),
    );

    let pair_providers =
        pool_aggregating::pair_providers(&args.shared.baseline_sources, chain_id, &web3).await;

    let mut base_tokens = HashSet::from_iter(args.shared.base_tokens);
    // We should always use the native token as a base token.
    base_tokens.insert(native_token.address());
    let mut allowed_tokens = args.shared.allowed_tokens;
    allowed_tokens.extend(base_tokens.iter().copied());
    let unsupported_tokens = args.shared.unsupported_tokens;

    let bad_token_detector = Arc::new(ListBasedDetector::new(
        allowed_tokens,
        unsupported_tokens,
        UnknownTokenStrategy::Allow,
    ));

    let current_block_stream = current_block_stream(web3.clone()).await.unwrap();

    let pool_aggregator = PoolAggregator::from_providers(&pair_providers, &web3).await;
    let pool_fetcher =
        CachedPoolFetcher::new(Box::new(pool_aggregator), current_block_stream.clone());

    let price_estimator = Arc::new(BaselinePriceEstimator::new(
        Box::new(pool_fetcher),
        gas_price_estimator.clone(),
        base_tokens,
        bad_token_detector.clone(),
        native_token.address(),
    ));
    let fee_calculator = Arc::new(EthAwareMinFeeCalculator::new(
        price_estimator.clone(),
        gas_price_estimator,
        native_token.address(),
        database.clone(),
        args.shared.fee_discount_factor,
        bad_token_detector.clone(),
    ));

    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        database.clone(),
        Box::new(balance_fetcher),
        fee_calculator.clone(),
        args.min_order_validity_period,
        bad_token_detector,
    ));
    let service_maintainer = ServiceMaintenance {
        maintainers: vec![
            orderbook.clone(),
            Arc::new(database.clone()),
            Arc::new(event_updater),
        ],
    };
    check_database_connection(orderbook.as_ref()).await;

    let registry = Registry::default();
    let metrics = Arc::new(Metrics::new(&registry).unwrap());

    let serve_task = serve_task(
        database.clone(),
        orderbook.clone(),
        fee_calculator,
        price_estimator,
        args.bind_address,
        registry,
        metrics.clone(),
    );
    let maintenance_task =
        task::spawn(service_maintainer.run_maintenance_on_new_block(current_block_stream));
    let db_metrics_task = task::spawn(database_metrics(metrics, database));

    tokio::select! {
        result = serve_task => tracing::error!(?result, "serve task exited"),
        result = maintenance_task => tracing::error!(?result, "maintenance task exited"),
        result = db_metrics_task => tracing::error!(?result, "database metrics task exited"),
    };
}

async fn check_database_connection(orderbook: &Orderbook) {
    orderbook
        .get_orders(&OrderFilter {
            uid: Some(OrderUid::default()),
            ..Default::default()
        })
        .await
        .expect("failed to connect to database");
}
