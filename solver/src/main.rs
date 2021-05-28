use contracts::{IUniswapLikeRouter, WETH9};
use ethcontract::{Account, PrivateKey, H160, U256};
use prometheus::Registry;
use reqwest::Url;
use shared::{
    amm_pair_provider::{SushiswapPairProvider, UniswapPairProvider},
    bad_token::list_based::ListBasedDetector,
    metrics::serve_metrics,
    network::network_name,
    pool_aggregating::{self, BaselineSources, PoolAggregator},
    price_estimate::BaselinePriceEstimator,
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    token_list::TokenList,
    transport::create_instrumented_transport,
};
use solver::{
    driver::Driver, liquidity::uniswap::UniswapLikeLiquidity,
    liquidity_collector::LiquidityCollector, metrics::Metrics, solver::SolverType,
};
use std::iter::FromIterator as _;
use std::{collections::HashSet, sync::Arc, time::Duration};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Arguments {
    #[structopt(flatten)]
    shared: shared::arguments::Arguments,

    /// The API endpoint to fetch the orderbook
    #[structopt(long, env = "ORDERBOOK_URL", default_value = "http://localhost:8080")]
    orderbook_url: Url,

    /// The API endpoint to call the mip solver
    #[structopt(long, env = "MIP_SOLVER_URL", default_value = "http://localhost:8000")]
    mip_solver_url: Url,

    /// The timeout for the API endpoint to fetch the orderbook
    #[structopt(
        long,
        env = "ORDERBOOK_TIMEOUT",
        default_value = "10",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    orderbook_timeout: Duration,

    /// The private key used by the driver to sign transactions.
    #[structopt(short = "k", long, env = "PRIVATE_KEY", hide_env_values = true)]
    private_key: PrivateKey,

    /// The target confirmation time for settlement transactions used to estimate gas price.
    #[structopt(
        long,
        env = "TARGET_CONFIRM_TIME",
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    target_confirm_time: Duration,

    /// Every how often we should execute the driver's run loop
    #[structopt(
        long,
        env = "SETTLE_INTERVAL",
        default_value = "10",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    settle_interval: Duration,

    /// Which type of solver to use
    #[structopt(
        long,
        env = "SOLVER_TYPE",
        default_value = "Naive,Baseline",
        possible_values = &SolverType::variants(),
        case_insensitive = true,
        use_delimiter = true,
    )]
    solvers: Vec<SolverType>,

    /// A settlement must contain at least one order older than this duration for it to be applied.
    /// Larger values delay individual settlements more but have a higher coincidence of wants
    /// chance.
    #[structopt(
        long,
        env = "MIN_ORDER_AGE",
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_order_age: Duration,

    /// The port at which we serve our metrics
    #[structopt(
        long,
        env = "METRICS_PORT",
        default_value = "9587",
        case_insensitive = true
    )]
    metrics_port: u16,

    /// The port at which we serve our metrics
    #[structopt(long, env = "MAX_MERGED_SETTLEMENTS", default_value = "5")]
    max_merged_settlements: usize,

    /// The maximum amount of time a solver is allowed to take.
    #[structopt(
        long,
        env = "SOLVER_TIME_LIMIT",
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    solver_time_limit: Duration,

    /// The minimum amount of sell volume (in ETH) that needs to be
    /// traded in order to use the 1Inch solver.
    #[structopt(
        long,
        env = "MIN_ORDER_SIZE_ONE_INCH",
        default_value = "5",
        parse(try_from_str = shared::arguments::wei_from_base_unit)
    )]
    min_order_size_one_inch: U256,

    /// The list of disabled 1Inch protocols. By default, the `PMM1` protocol
    /// (representing a private market maker) is disabled as it seems to
    /// produce invalid swaps.
    #[structopt(long, env, default_value = "PMM1", use_delimiter = true)]
    disabled_one_inch_protocols: Vec<String>,

    /// The list of tokens our settlement contract is willing to buy when settling trades
    /// without external liquidity
    #[structopt(
        long,
        env = "MARKET_MAKABLE_TOKEN_LIST",
        default_value = "https://tokens.coingecko.com/uniswap/all.json"
    )]
    market_makable_token_list: String,

    /// The maximum gas price the solver is willing to pay in a settlement
    #[structopt(
        long,
        env = "GAS_PRICE_CAP_GWEI",
        default_value = "1500",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    gas_price_cap: f64,
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    shared::tracing::initialize(args.shared.log_filter.as_str());
    tracing::info!("running solver with {:#?}", args);

    let registry = Registry::default();
    let metrics = Arc::new(Metrics::new(&registry).expect("Couldn't register metrics"));

    // TODO: custom transport that allows setting timeout
    let transport = create_instrumented_transport(
        web3::transports::Http::new(args.shared.node_url.as_str())
            .expect("transport creation failed"),
        metrics.clone(),
    );
    let web3 = web3::Web3::new(transport);
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    let network_id = web3
        .net()
        .version()
        .await
        .expect("failed to get network id");
    let network_name = network_name(&network_id, chain_id);
    let account = Account::Offline(args.private_key, Some(chain_id));
    let settlement_contract = solver::get_settlement_contract(&web3, account)
        .await
        .expect("couldn't load deployed settlement");
    let native_token_contract = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let orderbook_api = solver::orderbook::OrderBookApi::new(
        args.orderbook_url,
        args.orderbook_timeout,
        native_token_contract.clone(),
    );
    let mut base_tokens = HashSet::from_iter(args.shared.base_tokens);
    // We should always use the native token as a base token.
    base_tokens.insert(native_token_contract.address());

    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
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

    let pool_aggregator = PoolAggregator::from_providers(&pair_providers, &web3).await;

    // TODO: use caching pool fetcher
    let price_estimator = Arc::new(BaselinePriceEstimator::new(
        Box::new(pool_aggregator),
        gas_price_estimator.clone(),
        base_tokens.clone(),
        // Order book already filters bad tokens
        Arc::new(ListBasedDetector::deny_list(Vec::new())),
        native_token_contract.address(),
    ));
    let uniswap_like_liquidity = build_amm_artifacts(
        args.shared.baseline_sources,
        chain_id,
        settlement_contract.clone(),
        base_tokens.clone(),
        web3.clone(),
    )
    .await;
    let solver = solver::solver::create(
        args.solvers,
        base_tokens,
        native_token_contract.address(),
        args.mip_solver_url,
        &settlement_contract,
        token_info_fetcher,
        price_estimator.clone(),
        network_name.to_string(),
        chain_id,
        args.shared.fee_discount_factor,
        args.solver_time_limit,
        args.min_order_size_one_inch,
        args.disabled_one_inch_protocols,
    )
    .expect("failure creating solvers");
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity,
        orderbook_api,
    };
    let market_makable_token_list = TokenList::from_url(&args.market_makable_token_list, chain_id)
        .await
        .map_err(|err| tracing::error!("Couldn't fetch market makable token list: {}", err))
        .ok();
    let mut driver = Driver::new(
        settlement_contract,
        liquidity_collector,
        price_estimator,
        solver,
        gas_price_estimator,
        args.target_confirm_time,
        args.settle_interval,
        native_token_contract.address(),
        args.min_order_age,
        metrics,
        web3,
        network_id,
        args.max_merged_settlements,
        args.solver_time_limit,
        args.gas_price_cap,
        market_makable_token_list,
    );

    serve_metrics(registry, ([0, 0, 0, 0], args.metrics_port).into());
    driver.run_forever().await;
}

async fn build_amm_artifacts(
    sources: Vec<BaselineSources>,
    chain_id: u64,
    settlement_contract: contracts::GPv2Settlement,
    base_tokens: HashSet<H160>,
    web3: shared::Web3,
) -> Vec<UniswapLikeLiquidity> {
    let mut res = vec![];
    for source in sources {
        match source {
            BaselineSources::Uniswap => {
                let router = contracts::UniswapV2Router02::deployed(&web3)
                    .await
                    .expect("couldn't load deployed uniswap router");
                let factory = contracts::UniswapV2Factory::deployed(&web3)
                    .await
                    .expect("couldn't load deployed uniswap router");
                let pair_provider = Arc::new(UniswapPairProvider { factory, chain_id });
                res.push(UniswapLikeLiquidity::new(
                    IUniswapLikeRouter::at(&web3, router.address()),
                    pair_provider.clone(),
                    settlement_contract.clone(),
                    base_tokens.clone(),
                    web3.clone(),
                ));
            }
            BaselineSources::Sushiswap => {
                let router = contracts::SushiswapV2Router02::deployed(&web3)
                    .await
                    .expect("couldn't load deployed sushiswap router");
                let factory = contracts::SushiswapV2Factory::deployed(&web3)
                    .await
                    .expect("couldn't load deployed sushiswap router");
                let pair_provider = Arc::new(SushiswapPairProvider { factory });
                res.push(UniswapLikeLiquidity::new(
                    IUniswapLikeRouter::at(&web3, router.address()),
                    pair_provider.clone(),
                    settlement_contract.clone(),
                    base_tokens.clone(),
                    web3.clone(),
                ));
            }
        }
    }
    res
}
