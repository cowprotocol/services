use anyhow::anyhow;
use contracts::{IUniswapLikeRouter, WETH9};
use ethcontract::{Account, PrivateKey, H160, U256};
use reqwest::Url;
use shared::{
    bad_token::list_based::ListBasedDetector,
    baseline_solver::BaseTokens,
    current_block::current_block_stream,
    maintenance::{Maintaining, ServiceMaintenance},
    metrics::{serve_metrics, setup_metrics_registry},
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
    transport::{create_instrumented_transport, http::HttpTransport},
    zeroex_api::DefaultZeroExApi,
};
use solver::{
    driver::Driver,
    liquidity::{
        balancer::BalancerV2Liquidity, order_converter::OrderConverter,
        uniswap::UniswapLikeLiquidity,
    },
    liquidity_collector::LiquidityCollector,
    metrics::Metrics,
    orderbook::OrderBookApi,
    settlement_submission::{
        archer_api::ArcherApi, flashbots_api::FlashbotsApi, SolutionSubmitter, TransactionStrategy,
    },
    solver::SolverType,
};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};
use structopt::{clap::arg_enum, StructOpt};

#[derive(Debug, StructOpt)]
struct Arguments {
    #[structopt(flatten)]
    shared: shared::arguments::Arguments,

    /// The API endpoint to fetch the orderbook
    #[structopt(long, env, default_value = "http://localhost:8080")]
    orderbook_url: Url,

    /// The API endpoint to call the mip solver
    #[structopt(long, env, default_value = "http://localhost:8000")]
    mip_solver_url: Url,

    /// The API endpoint to call the mip v2 solver
    #[structopt(long, env, default_value = "http://localhost:8000")]
    quasimodo_solver_url: Url,

    /// The account used by the driver to sign transactions. This can be either
    /// a 32-byte private key for offline signing, or a 20-byte Ethereum address
    /// for signing with a local node account.
    #[structopt(long, env, hide_env_values = true)]
    solver_account: Option<SolverAccountArg>,

    /// The target confirmation time in seconds for settlement transactions used to estimate gas price.
    #[structopt(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    target_confirm_time: Duration,

    /// Specify the interval in seconds between consecutive driver run loops.
    ///
    /// This is typically a low value to prevent busy looping in case of some
    /// internal driver error, but can be set to a larger value for running
    /// drivers in dry-run mode to prevent repeatedly settling the same
    /// orders.
    #[structopt(
        long,
        env,
        default_value = "1",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    settle_interval: Duration,

    /// Which type of solver to use
    #[structopt(
        long,
        env,
        default_value = "Naive,Baseline",
        possible_values = &SolverType::variants(),
        case_insensitive = true,
        use_delimiter = true,
    )]
    solvers: Vec<SolverType>,

    /// Individual accounts for each solver. See `--solver-account` for more
    /// information about configuring accounts.
    #[structopt(
        long,
        env,
        case_insensitive = true,
        use_delimiter = true,
        hide_env_values = true
    )]
    solver_accounts: Option<Vec<SolverAccountArg>>,

    /// A settlement must contain at least one order older than this duration in seconds for it
    /// to be applied.  Larger values delay individual settlements more but have a higher
    /// coincidence of wants chance.
    #[structopt(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_order_age: Duration,

    /// The port at which we serve our metrics
    #[structopt(long, env, default_value = "9587", case_insensitive = true)]
    metrics_port: u16,

    /// The port at which we serve our metrics
    #[structopt(long, env, default_value = "5")]
    max_merged_settlements: usize,

    /// The maximum amount of time in seconds a solver is allowed to take.
    #[structopt(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    solver_time_limit: Duration,

    /// The minimum amount of sell volume (in ETH) that needs to be
    /// traded in order to use the 1Inch solver.
    #[structopt(
        long,
        env,
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
        env,
        default_value = "https://tokens.coingecko.com/uniswap/all.json"
    )]
    market_makable_token_list: String,

    /// The maximum gas price in Gwei the solver is willing to pay in a settlement.
    #[structopt(
        long,
        env,
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

    /// The maximum time in seconds we spend trying to settle a transaction through the archer
    /// network before going to back to solving.
    #[structopt(
        long,
        default_value = "60",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    max_archer_submission_seconds: Duration,

    /// The maximum time in seconds we spend trying to settle a transaction through the flashbots
    /// network before going to back to solving.
    #[structopt(
        long,
        default_value = "120",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    max_flashbots_submission_seconds: Duration,

    /// The RPC endpoints to use for submitting transaction to a custom set of nodes.
    #[structopt(long, env, use_delimiter = true)]
    transaction_submission_nodes: Vec<Url>,

    /// The configured addresses whose orders should be considered liquidity
    /// and not to be included in the objective function by the HTTP solver.
    #[structopt(long, env, use_delimiter = true)]
    liquidity_order_owners: Vec<H160>,

    /// Fee scaling factor for objective value. This controls the constant
    /// factor by which order fees are multiplied with. Setting this to a value
    /// greater than 1.0 makes settlements with negative objective values less
    /// likely, promoting more aggressive merging of single order settlements.
    #[structopt(long, env, default_value = "1", parse(try_from_str = shared::arguments::parse_fee_factor))]
    fee_objective_scaling_factor: f64,

    /// The maximum number of settlements the driver considers per solver.
    #[structopt(long, env, default_value = "20")]
    max_settlements_per_solver: usize,
}

arg_enum! {
    #[derive(Debug)]
    enum TransactionStrategyArg {
        PublicMempool,
        ArcherNetwork,
        Flashbots,
        CustomNodes,
        DryRun,
    }
}

#[derive(Debug)]
enum SolverAccountArg {
    PrivateKey(PrivateKey),
    Address(H160),
}

impl SolverAccountArg {
    fn into_account(self, chain_id: u64) -> Account {
        match self {
            SolverAccountArg::PrivateKey(key) => Account::Offline(key, Some(chain_id)),
            SolverAccountArg::Address(address) => Account::Local(address, None),
        }
    }
}

impl FromStr for SolverAccountArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<PrivateKey>()
            .map(SolverAccountArg::PrivateKey)
            .or_else(|pk_err| {
                Ok(SolverAccountArg::Address(s.parse().map_err(
                    |addr_err| {
                        anyhow!("could not parse as private key: {}", pk_err)
                            .context(anyhow!("could not parse as address: {}", addr_err))
                            .context("invalid solver account, it is neither a private key or an Ethereum address")
                    },
                )?))
            })
    }
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    shared::tracing::initialize(
        args.shared.log_filter.as_str(),
        args.shared.log_stderr_threshold,
    );
    tracing::info!("running solver with validated {:#?}", args);

    setup_metrics_registry(Some("gp_v2_solver".into()), None);
    let metrics = Arc::new(Metrics::new().expect("Couldn't register metrics"));

    let client = shared::http_client(args.shared.http_timeout);

    let transport = create_instrumented_transport(
        HttpTransport::new(client.clone(), args.shared.node_url, "base".to_string()),
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
            args.shared.blocknative_api_key,
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
        if let Some(solver_accounts) = args.solver_accounts {
            assert!(
                solver_accounts.len() == args.solvers.len(),
                "number of solvers ({}) does not match the number of accounts ({})",
                args.solvers.len(),
                solver_accounts.len()
            );

            solver_accounts
                .into_iter()
                .map(|account_arg| account_arg.into_account(chain_id))
                .zip(args.solvers)
                .collect()
        } else if let Some(account_arg) = args.solver_account {
            std::iter::repeat(account_arg.into_account(chain_id))
                .zip(args.solvers)
                .collect()
        } else {
            panic!("either SOLVER_ACCOUNTS or SOLVER_ACCOUNT must be set")
        }
    };

    let zeroex_api = Arc::new(
        DefaultZeroExApi::new(
            args.shared
                .zeroex_url
                .as_deref()
                .unwrap_or(DefaultZeroExApi::DEFAULT_URL),
            args.shared.zeroex_api_key,
            client.clone(),
        )
        .unwrap(),
    );

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
        args.min_order_size_one_inch,
        args.disabled_one_inch_protocols,
        args.paraswap_slippage_bps,
        args.shared.disabled_paraswap_dexs,
        args.shared.paraswap_partner,
        client.clone(),
        native_token_price_estimation_amount,
        metrics.clone(),
        zeroex_api,
    )
    .expect("failure creating solvers");
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity,
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
            TransactionStrategyArg::Flashbots => TransactionStrategy::Flashbots {
                flashbots_api: FlashbotsApi::new(client.clone()),
                max_confirm_time: args.max_flashbots_submission_seconds,
            },
            TransactionStrategyArg::CustomNodes => {
                assert!(
                    !args.transaction_submission_nodes.is_empty(),
                    "missing transaction submission nodes"
                );
                let nodes = args
                    .transaction_submission_nodes
                    .into_iter()
                    .enumerate()
                    .map(|(index, url)| {
                        let transport = create_instrumented_transport(
                            HttpTransport::new(client.clone(), url, index.to_string()),
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
    let api = OrderBookApi::new(args.orderbook_url, client.clone());
    let order_converter = OrderConverter {
        native_token: native_token_contract.clone(),
        liquidity_order_owners: args.liquidity_order_owners.into_iter().collect(),
        fee_objective_scaling_factor: args.fee_objective_scaling_factor,
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
        solution_submitter,
        native_token_price_estimation_amount,
        args.max_settlements_per_solver,
        api,
        order_converter,
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

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq for SolverAccountArg {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (SolverAccountArg::PrivateKey(a), SolverAccountArg::PrivateKey(b)) => {
                    a.public_address() == b.public_address()
                }
                (SolverAccountArg::Address(a), SolverAccountArg::Address(b)) => a == b,
                _ => false,
            }
        }
    }

    #[test]
    fn parses_solver_account_arg() {
        assert_eq!(
            "0x4242424242424242424242424242424242424242424242424242424242424242"
                .parse::<SolverAccountArg>()
                .unwrap(),
            SolverAccountArg::PrivateKey(PrivateKey::from_raw([0x42; 32]).unwrap())
        );
        assert_eq!(
            "0x4242424242424242424242424242424242424242"
                .parse::<SolverAccountArg>()
                .unwrap(),
            SolverAccountArg::Address(H160([0x42; 20])),
        );
    }

    #[test]
    fn errors_on_invalid_solver_account_arg() {
        assert!("0x010203040506070809101112131415161718192021"
            .parse::<SolverAccountArg>()
            .is_err());
        assert!("not an account".parse::<SolverAccountArg>().is_err());
    }
}
