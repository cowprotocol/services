use contracts::{IUniswapLikeRouter, WETH9};
use ethcontract::{Account, PrivateKey, H160, U256};
use reqwest::Url;
use shared::baseline_solver::BaseTokens;
use shared::metrics::setup_metrics_registry;
use shared::{
    bad_token::list_based::ListBasedDetector,
    current_block::current_block_stream,
    maintenance::{Maintaining, ServiceMaintenance},
    metrics::serve_metrics,
    network::network_name,
    price_estimation::baseline::BaselinePriceEstimator,
    recent_block_cache::CacheConfig,
    sources::{
        self,
        balancer::pool_fetching::BalancerPoolFetcher,
        uniswap::{
            pool_cache::PoolCache,
            pool_fetching::{PoolFetcher, PoolFetching},
        },
        BaselineSource, PoolAggregator,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    token_list::TokenList,
    transport::create_instrumented_transport,
    transport::http::HttpTransport,
};
use solver::{
    driver::Driver,
    liquidity::{balancer::BalancerV2Liquidity, uniswap::UniswapLikeLiquidity},
    liquidity_collector::LiquidityCollector,
    metrics::Metrics,
    settlement_submission::{archer_api::ArcherApi, SolutionSubmitter, TransactionStrategy},
    solver::SolverType,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use structopt::{clap::arg_enum, StructOpt};

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

    /// The API endpoint to call the mip v2 solver
    #[structopt(
        long,
        env = "QUASIMODO_SOLVER_URL",
        default_value = "http://localhost:8000"
    )]
    quasimodo_solver_url: Url,

    /// The private key used by the driver to sign transactions.
    #[structopt(short = "k", long, env = "PRIVATE_KEY", hide_env_values = true)]
    private_key: Option<PrivateKey>,

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

    /// Individual private keys for each solver
    #[structopt(
        long,
        env = "SOLVER_PRIVATE_KEYS",
        case_insensitive = true,
        use_delimiter = true,
        hide_env_values = true
    )]
    solver_private_keys: Option<Vec<PrivateKey>>,

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

    /// The slippage tolerance we apply to the price quoted by Paraswap
    #[structopt(long, env, default_value = "10")]
    paraswap_slippage_bps: u32,

    /// The authorization for the archer api.
    #[structopt(long, env)]
    archer_authorization: Option<String>,

    /// How to to submit settlement transactions.
    #[structopt(long, env, default_value = "PublicMempool")]
    transaction_strategy: TransactionStrategyArg,

    /// The maximum time we spend trying to settle a transaction through the archer network before
    /// going to back to solving.
    #[structopt(
        long,
        default_value = "60",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    max_archer_submission_seconds: Duration,

    /// The RPC endpoints to use for submitting transaction to a custom set of nodes.
    #[structopt(long, env, use_delimiter = true)]
    transaction_submission_nodes: Vec<Url>,

    /// The configured addresses whose orders should be considered liquidity
    /// and not to be included in the objective function by the HTTP solver.
    #[structopt(long, env, use_delimiter = true)]
    liquidity_order_owners: Vec<H160>,
}

arg_enum! {
    #[derive(Debug)]
    pub enum TransactionStrategyArg {
        PublicMempool,
        ArcherNetwork,
        CustomNodes,
        DryRun,
    }
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    shared::tracing::initialize(args.shared.log_filter.as_str());
    tracing::info!("running solver with validated {:#?}", args);

    setup_metrics_registry(Some("gp_v2_solver".into()), None);
    let metrics = Arc::new(Metrics::new().expect("Couldn't register metrics"));

    let client = shared::http_client(args.shared.http_timeout);

    let transport = create_instrumented_transport(
        HttpTransport::new(client.clone(), args.shared.node_url),
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
    let settlement_contract = solver::get_settlement_contract(&web3)
        .await
        .expect("couldn't load deployed settlement");
    let native_token_contract = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let orderbook_api = solver::orderbook::OrderBookApi::new(
        args.orderbook_url,
        native_token_contract.clone(),
        client.clone(),
        args.liquidity_order_owners.into_iter().collect(),
    );

    let base_tokens = Arc::new(BaseTokens::new(
        native_token_contract.address(),
        &args.shared.base_tokens,
    ));

    let native_token_price_estimation_amount = args
        .shared
        .amount_to_estimate_prices_with
        .or_else(|| shared::arguments::default_amount_to_estimate_prices_with(&network_id))
        .expect("No amount to estimate prices with set.");

    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            client.clone(),
            &web3,
            args.shared.gas_estimators.as_slice(),
        )
        .await
        .expect("failed to create gas price estimator"),
    );

    let current_block_stream =
        current_block_stream(web3.clone(), args.shared.block_stream_poll_interval_seconds)
            .await
            .unwrap();

    let cache_config = CacheConfig {
        number_of_blocks_to_cache: args.shared.pool_cache_blocks,
        // 0 because we don't make use of the auto update functionality as we always fetch
        // for specific blocks
        number_of_entries_to_auto_update: 0,
        maximum_recent_block_age: args.shared.pool_cache_maximum_recent_block_age,
        max_retries: args.shared.pool_cache_maximum_retries,
        delay_between_retries: args.shared.pool_cache_delay_between_retries_seconds,
    };
    let pool_caches: HashMap<BaselineSource, Arc<PoolCache>> =
        sources::pair_providers(&args.shared.baseline_sources, chain_id, &web3)
            .await
            .into_iter()
            .map(|(source, pair_provider)| {
                let fetcher = Box::new(PoolFetcher {
                    pair_provider,
                    web3: web3.clone(),
                });
                let pool_cache = PoolCache::new(
                    cache_config,
                    fetcher,
                    current_block_stream.clone(),
                    metrics.clone(),
                )
                .expect("failed to create pool cache");
                (source, Arc::new(pool_cache))
            })
            .collect();

    let pool_aggregator = Arc::new(PoolAggregator {
        pool_fetchers: pool_caches
            .values()
            .map(|cache| cache.clone() as Arc<dyn PoolFetching>)
            .collect(),
    });

    let (balancer_pool_maintainer, balancer_v2_liquidity) = if args
        .shared
        .baseline_sources
        .contains(&BaselineSource::BalancerV2)
    {
        let balancer_pool_fetcher = Arc::new(
            BalancerPoolFetcher::new(
                chain_id,
                web3.clone(),
                token_info_fetcher.clone(),
                cache_config,
                current_block_stream.clone(),
                metrics.clone(),
                client.clone(),
            )
            .await
            .expect("failed to create Balancer pool fetcher"),
        );
        (
            Some(balancer_pool_fetcher.clone() as Arc<dyn Maintaining>),
            Some(
                BalancerV2Liquidity::new(web3.clone(), balancer_pool_fetcher, base_tokens.clone())
                    .await
                    .expect("failed to create Balancer V2 liquidity"),
            ),
        )
    } else {
        (None, None)
    };

    let price_estimator = Arc::new(BaselinePriceEstimator::new(
        pool_aggregator,
        gas_price_estimator.clone(),
        base_tokens.clone(),
        // Order book already filters bad tokens
        Arc::new(ListBasedDetector::deny_list(Vec::new())),
        native_token_contract.address(),
        native_token_price_estimation_amount,
    ));
    let uniswap_like_liquidity = build_amm_artifacts(
        &pool_caches,
        settlement_contract.clone(),
        base_tokens.clone(),
        web3.clone(),
    )
    .await;

    let solvers = {
        if let Some(private_keys) = args.solver_private_keys {
            assert!(
                private_keys.len() == args.solvers.len(),
                "number of solver does not match the number of private keys"
            );

            private_keys
                .into_iter()
                .map(|private_key| Account::Offline(private_key, Some(chain_id)))
                .zip(args.solvers)
                .collect()
        } else if let Some(private_key) = args.private_key {
            std::iter::repeat(Account::Offline(private_key, Some(chain_id)))
                .zip(args.solvers)
                .collect()
        } else {
            panic!("either SOLVER_PRIVATE_KEY or PRIVATE_KEY must be set")
        }
    };

    let solver = solver::solver::create(
        web3.clone(),
        solvers,
        base_tokens,
        native_token_contract.address(),
        args.mip_solver_url,
        args.quasimodo_solver_url,
        &settlement_contract,
        token_info_fetcher,
        price_estimator.clone(),
        network_name.to_string(),
        chain_id,
        args.shared.fee_factor,
        args.min_order_size_one_inch,
        args.disabled_one_inch_protocols,
        args.paraswap_slippage_bps,
        args.shared.disabled_paraswap_dexs,
        args.shared.paraswap_partner,
        client.clone(),
        native_token_price_estimation_amount,
    )
    .expect("failure creating solvers");
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity,
        orderbook_api,
        balancer_v2_liquidity,
    };
    let market_makable_token_list =
        TokenList::from_url(&args.market_makable_token_list, chain_id, client.clone())
            .await
            .map_err(|err| tracing::error!("Couldn't fetch market makable token list: {}", err))
            .ok();
    let solution_submitter = SolutionSubmitter {
        web3: web3.clone(),
        contract: settlement_contract.clone(),
        gas_price_estimator: gas_price_estimator.clone(),
        target_confirm_time: args.target_confirm_time,
        gas_price_cap: args.gas_price_cap,
        transaction_strategy: match args.transaction_strategy {
            TransactionStrategyArg::PublicMempool => {
                TransactionStrategy::CustomNodes(vec![web3.clone()])
            }
            TransactionStrategyArg::ArcherNetwork => TransactionStrategy::ArcherNetwork {
                archer_api: ArcherApi::new(
                    args.archer_authorization
                        .expect("missing archer authorization"),
                    client.clone(),
                ),
                max_confirm_time: args.max_archer_submission_seconds,
            },
            TransactionStrategyArg::CustomNodes => {
                assert!(
                    !args.transaction_submission_nodes.is_empty(),
                    "missing transaction submission nodes"
                );
                let nodes = args
                    .transaction_submission_nodes
                    .into_iter()
                    .map(|url| {
                        let transport = create_instrumented_transport(
                            HttpTransport::new(client.clone(), url),
                            metrics.clone(),
                        );
                        web3::Web3::new(transport)
                    })
                    .collect::<Vec<_>>();
                for node in &nodes {
                    let node_network_id = node.net().version().await.unwrap();
                    assert_eq!(
                        node_network_id, network_id,
                        "network id of custom node doesn't match main node"
                    );
                }
                TransactionStrategy::CustomNodes(nodes)
            }
            TransactionStrategyArg::DryRun => TransactionStrategy::DryRun,
        },
    };
    let mut driver = Driver::new(
        settlement_contract,
        liquidity_collector,
        price_estimator,
        solver,
        gas_price_estimator,
        args.settle_interval,
        native_token_contract.address(),
        args.min_order_age,
        metrics.clone(),
        web3,
        network_id,
        args.max_merged_settlements,
        args.solver_time_limit,
        market_makable_token_list,
        current_block_stream.clone(),
        args.shared.fee_factor,
        solution_submitter,
        native_token_price_estimation_amount,
    );

    let maintainer = ServiceMaintenance {
        maintainers: pool_caches
            .into_iter()
            .map(|(_, cache)| cache as Arc<dyn Maintaining>)
            .chain(balancer_pool_maintainer)
            .collect(),
    };
    tokio::task::spawn(maintainer.run_maintenance_on_new_block(current_block_stream));

    serve_metrics(metrics, ([0, 0, 0, 0], args.metrics_port).into());
    driver.run_forever().await;
}

async fn build_amm_artifacts(
    sources: &HashMap<BaselineSource, Arc<PoolCache>>,
    settlement_contract: contracts::GPv2Settlement,
    base_tokens: Arc<BaseTokens>,
    web3: shared::Web3,
) -> Vec<UniswapLikeLiquidity> {
    let mut res = vec![];
    for (key, value) in sources {
        match key {
            BaselineSource::Uniswap => {
                let router = contracts::UniswapV2Router02::deployed(&web3)
                    .await
                    .expect("couldn't load deployed uniswap router");
                res.push(UniswapLikeLiquidity::new(
                    IUniswapLikeRouter::at(&web3, router.address()),
                    settlement_contract.clone(),
                    base_tokens.clone(),
                    web3.clone(),
                    value.clone(),
                ));
            }
            BaselineSource::Sushiswap => {
                let router = contracts::SushiswapV2Router02::deployed(&web3)
                    .await
                    .expect("couldn't load deployed sushiswap router");
                res.push(UniswapLikeLiquidity::new(
                    IUniswapLikeRouter::at(&web3, router.address()),
                    settlement_contract.clone(),
                    base_tokens.clone(),
                    web3.clone(),
                    value.clone(),
                ));
            }
            BaselineSource::BalancerV2 => (),
        }
    }
    res
}
