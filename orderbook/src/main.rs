use chrono::offset::Utc;
use contracts::{GPv2Settlement, UniswapV2Factory, WETH9};
use futures::StreamExt;
use model::{order::OrderUid, DomainSeparator};
use orderbook::{
    account_balances::Web3BalanceFetcher,
    database::{Database, OrderFilter},
    event_updater::EventUpdater,
    fee::MinFeeCalculator,
    orderbook::Orderbook,
    serve_task, verify_deployed_contract_constants,
};
use shared::{
    amm_pair_provider::UniswapPairProvider,
    current_block::{current_block_stream, CurrentBlockStream},
    pool_fetching::{CachedPoolFetcher, FilteredPoolFetcher, PoolFetcher},
    price_estimate::UniswapPriceEstimator,
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

    #[structopt(long, env = "FEE_DISCOUNT_FACTOR", default_value = "1")]
    fee_discount_factor: f64,

    /// The minimum amount of time an order has to be valid for.
    #[structopt(
        long,
        env = "MIN_ORDER_VALIDITY_PERIOD",
        default_value = "60",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_order_validity_period: Duration,
}

pub async fn orderbook_maintenance(
    storage: Arc<Orderbook>,
    database: Database,
    settlement_contract: GPv2Settlement,
    mut current_block_stream: CurrentBlockStream,
) -> ! {
    while let Some(block) = current_block_stream.next().await {
        tracing::debug!(
            "running maintenance on block number {:?} hash {:?}",
            block.number,
            block.hash
        );
        if let Err(err) = storage.run_maintenance(&settlement_contract).await {
            tracing::error!(?err, "orderbook maintenance error");
        }
        if let Err(err) = database.remove_expired_fee_measurements(Utc::now()).await {
            tracing::error!(?err, "fee measurement maintenance error");
        }
    }
    unreachable!()
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
    let uniswap_factory = UniswapV2Factory::deployed(&web3)
        .await
        .expect("couldn't load deployed uniswap factory");
    let native_token = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    let uniswap_pair_provider = UniswapPairProvider {
        factory: uniswap_factory,
        chain_id,
    };
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
    let balance_fetcher =
        Web3BalanceFetcher::new(web3.clone(), gp_allowance, settlement_contract.address());

    let gas_price_estimator = shared::gas_price_estimation::create_priority_estimator(
        &reqwest::Client::new(),
        &web3,
        args.shared.gas_estimators.as_slice(),
    )
    .await
    .expect("failed to create gas price estimator");

    let unsupported_tokens = HashSet::from_iter(args.shared.unsupported_tokens);
    let mut base_tokens = HashSet::from_iter(args.shared.base_tokens);
    // We should always use the native token as a base token.
    base_tokens.insert(native_token.address());
    assert!(
        unsupported_tokens
            .intersection(&base_tokens)
            .collect::<HashSet<_>>()
            .is_empty(),
        "Base tokens include at least one unsupported token!"
    );

    let current_block_stream = current_block_stream(web3.clone()).await.unwrap();
    let cached_pool_fetcher = CachedPoolFetcher::new(
        Box::new(PoolFetcher {
            pair_provider: Arc::new(uniswap_pair_provider),
            web3,
        }),
        current_block_stream.clone(),
    );
    let pool_fetcher =
        FilteredPoolFetcher::new(Box::new(cached_pool_fetcher), unsupported_tokens.clone());
    let price_estimator = Arc::new(UniswapPriceEstimator::new(
        Box::new(pool_fetcher),
        base_tokens,
    ));
    let fee_calculator = Arc::new(MinFeeCalculator::new(
        price_estimator.clone(),
        Box::new(gas_price_estimator),
        native_token.address(),
        database.clone(),
        args.fee_discount_factor,
        unsupported_tokens.clone(),
    ));

    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        database.clone(),
        event_updater,
        Box::new(balance_fetcher),
        fee_calculator.clone(),
        unsupported_tokens,
        args.min_order_validity_period,
    ));
    check_database_connection(orderbook.as_ref()).await;

    let serve_task = serve_task(
        database.clone(),
        orderbook.clone(),
        fee_calculator,
        price_estimator,
        args.bind_address,
    );
    let maintenance_task = task::spawn(orderbook_maintenance(
        orderbook,
        database,
        settlement_contract,
        current_block_stream,
    ));
    tokio::select! {
        result = serve_task => tracing::error!(?result, "serve task exited"),
        result = maintenance_task => tracing::error!(?result, "maintenance task exited"),
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
