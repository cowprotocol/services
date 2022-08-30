use anyhow::{Context, Result};
use clap::Parser;
use contracts::{IUniswapLikeRouter, UniswapV3SwapRouter, WETH9};
use driver::{
    api::serve_api, arguments::Arguments, auction_converter::AuctionConverter,
    commit_reveal::CommitRevealSolver, driver::Driver,
};
use gas_estimation::GasPriceEstimating;
use reqwest::Client;
use shared::{
    baseline_solver::BaseTokens,
    current_block::{current_block_stream, CurrentBlockStream},
    http_solver::{DefaultHttpSolverApi, SolverConfig},
    maintenance::{Maintaining, ServiceMaintenance},
    recent_block_cache::CacheConfig,
    sources::{
        self,
        balancer_v2::{pool_fetching::BalancerContracts, BalancerFactoryKind, BalancerPoolFetcher},
        uniswap_v2::pool_cache::PoolCache,
        uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
        BaselineSource,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher, TokenInfoFetching},
    transport::http::HttpTransport,
    zeroex_api::DefaultZeroExApi,
    Web3Transport,
};
use solver::{
    arguments::TransactionStrategyArg,
    driver_logger::DriverLogger,
    interactions::allowances::AllowanceManager,
    liquidity::{
        balancer_v2::BalancerV2Liquidity, order_converter::OrderConverter,
        uniswap_v2::UniswapLikeLiquidity, uniswap_v3::UniswapV3Liquidity, zeroex::ZeroExLiquidity,
    },
    liquidity_collector::LiquidityCollector,
    metrics::Metrics,
    settlement_access_list::AccessListEstimating,
    settlement_ranker::SettlementRanker,
    settlement_rater::SettlementRater,
    settlement_simulation::TenderlyApi,
    settlement_submission::{
        submitter::{
            custom_nodes_api::CustomNodesApi, eden_api::EdenApi, flashbots_api::FlashbotsApi,
            Strategy,
        },
        GlobalTxPool, SolutionSubmitter, StrategyArgs, TransactionStrategy,
    },
    solver::{
        http_solver::{buffers::BufferRetriever, HttpSolver, InstanceCache},
        Solver,
    },
};
use std::{collections::HashMap, sync::Arc, time::Duration};

struct CommonComponents {
    client: Client,
    web3: shared::Web3,
    network_id: String,
    chain_id: u64,
    settlement_contract: contracts::GPv2Settlement,
    native_token_contract: WETH9,
    access_list_estimator: Arc<dyn AccessListEstimating>,
    gas_price_estimator: Arc<dyn GasPriceEstimating>,
    order_converter: Arc<OrderConverter>,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    current_block_stream: CurrentBlockStream,
}

async fn init_common_components(args: &Arguments) -> CommonComponents {
    let client = shared::http_client(args.http_timeout);
    let transport = Web3Transport::new(HttpTransport::new(
        client.clone(),
        args.node_url.clone(),
        "base".to_string(),
    ));
    let web3 = web3::Web3::new(transport);
    let network_id = web3
        .net()
        .version()
        .await
        .expect("failed to get network id");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    let settlement_contract = solver::get_settlement_contract(&web3)
        .await
        .expect("couldn't load deployed settlement");
    let native_token_contract = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let access_list_estimator = Arc::new(
        solver::settlement_access_list::create_priority_estimator(
            &client,
            &web3,
            args.access_list_estimators.as_slice(),
            args.tenderly_url.clone(),
            args.tenderly_api_key.clone(),
            network_id.clone(),
        )
        .await
        .expect("failed to create access list estimator"),
    );
    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            client.clone(),
            &web3,
            args.gas_estimators.as_slice(),
            args.blocknative_api_key.clone(),
        )
        .await
        .expect("failed to create gas price estimator"),
    );
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let current_block_stream =
        current_block_stream(web3.clone(), args.block_stream_poll_interval_seconds)
            .await
            .unwrap();

    let order_converter = Arc::new(OrderConverter {
        native_token: native_token_contract.clone(),
        fee_objective_scaling_factor: args.fee_objective_scaling_factor,
    });

    CommonComponents {
        client,
        web3,
        network_id,
        chain_id,
        settlement_contract,
        native_token_contract,
        access_list_estimator,
        gas_price_estimator,
        order_converter,
        token_info_fetcher,
        current_block_stream,
    }
}

async fn build_solvers(common: &CommonComponents, args: &Arguments) -> Vec<Arc<dyn Solver>> {
    let buffer_retriever = Arc::new(BufferRetriever::new(
        common.web3.clone(),
        common.settlement_contract.address(),
    ));
    let allowance_mananger = Arc::new(AllowanceManager::new(
        common.web3.clone(),
        common.settlement_contract.address(),
    ));
    let http_solver_cache = InstanceCache::default();

    args.solvers
        .iter()
        .map(|arg| {
            Arc::new(HttpSolver::new(
                DefaultHttpSolverApi {
                    name: arg.name.clone(),
                    network_name: common.network_id.clone(),
                    chain_id: common.chain_id,
                    base: arg.url.clone(),
                    client: common.client.clone(),
                    config: SolverConfig {
                        use_internal_buffers: Some(args.use_internal_buffers),
                        ..Default::default()
                    },
                },
                arg.account.clone().into_account(common.chain_id),
                common.native_token_contract.address(),
                common.token_info_fetcher.clone(),
                buffer_retriever.clone(),
                allowance_mananger.clone(),
                common.order_converter.clone(),
                http_solver_cache.clone(),
                false,
            )) as Arc<dyn Solver>
        })
        .collect()
}

async fn build_submitter(common: &CommonComponents, args: &Arguments) -> Arc<SolutionSubmitter> {
    let client = &common.client;
    let web3 = &common.web3;

    let submission_nodes_with_url = args
        .transaction_submission_nodes
        .iter()
        .enumerate()
        .map(|(index, url)| {
            let transport = Web3Transport::new(HttpTransport::new(
                client.clone(),
                url.clone(),
                index.to_string(),
            ));
            (web3::Web3::new(transport), url)
        })
        .collect::<Vec<_>>();
    for (node, url) in &submission_nodes_with_url {
        let node_network_id = node
            .net()
            .version()
            .await
            .with_context(|| {
                format!(
                    "Unable to retrieve network id on startup using the submission node at {url}"
                )
            })
            .unwrap();
        assert_eq!(
            node_network_id, common.network_id,
            "network id of custom node doesn't match main node"
        );
    }
    let submission_nodes = submission_nodes_with_url
        .into_iter()
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    let submitted_transactions = GlobalTxPool::default();
    let mut transaction_strategies = vec![];
    for strategy in &args.transaction_strategy {
        match strategy {
            TransactionStrategyArg::PublicMempool => {
                transaction_strategies.push(TransactionStrategy::CustomNodes(StrategyArgs {
                    submit_api: Box::new(CustomNodesApi::new(
                        vec![web3.clone()],
                        args.disable_high_risk_public_mempool_transactions,
                    )),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::CustomNodes),
                }))
            }
            TransactionStrategyArg::Eden => {
                transaction_strategies.push(TransactionStrategy::Eden(StrategyArgs {
                    submit_api: Box::new(
                        EdenApi::new(
                            client.clone(),
                            args.eden_api_url.clone(),
                            submitted_transactions.clone(),
                        )
                        .unwrap(),
                    ),
                    max_additional_tip: args.max_additional_eden_tip,
                    additional_tip_percentage_of_max_fee: args.additional_tip_percentage,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::Eden),
                }))
            }
            TransactionStrategyArg::Flashbots => {
                for flashbots_url in args.flashbots_api_url.clone() {
                    transaction_strategies.push(TransactionStrategy::Flashbots(StrategyArgs {
                        submit_api: Box::new(
                            FlashbotsApi::new(client.clone(), flashbots_url).unwrap(),
                        ),
                        max_additional_tip: args.max_additional_flashbot_tip,
                        additional_tip_percentage_of_max_fee: args.additional_tip_percentage,
                        sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::Flashbots),
                    }))
                }
            }
            TransactionStrategyArg::CustomNodes => {
                assert!(
                    !submission_nodes.is_empty(),
                    "missing transaction submission nodes"
                );
                transaction_strategies.push(TransactionStrategy::CustomNodes(StrategyArgs {
                    submit_api: Box::new(CustomNodesApi::new(
                        submission_nodes.clone(),
                        args.disable_high_risk_public_mempool_transactions,
                    )),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::CustomNodes),
                }))
            }
            TransactionStrategyArg::DryRun => {
                transaction_strategies.push(TransactionStrategy::DryRun)
            }
        }
    }

    Arc::new(SolutionSubmitter {
        web3: web3.clone(),
        contract: common.settlement_contract.clone(),
        gas_price_estimator: common.gas_price_estimator.clone(),
        target_confirm_time: args.target_confirm_time,
        max_confirm_time: args.max_submission_seconds,
        retry_interval: args.submission_retry_interval_seconds,
        gas_price_cap: args.gas_price_cap,
        transaction_strategies,
        access_list_estimator: common.access_list_estimator.clone(),
    })
}

async fn build_auction_converter(
    common: &CommonComponents,
    args: &Arguments,
) -> Result<Arc<AuctionConverter>> {
    let base_tokens = Arc::new(BaseTokens::new(
        common.native_token_contract.address(),
        &args.base_tokens,
    ));
    let cache_config = CacheConfig {
        number_of_blocks_to_cache: args.pool_cache_blocks,
        // 0 because we don't make use of the auto update functionality as we always fetch
        // for specific blocks
        number_of_entries_to_auto_update: 0,
        maximum_recent_block_age: args.pool_cache_maximum_recent_block_age,
        max_retries: args.pool_cache_maximum_retries,
        delay_between_retries: args.pool_cache_delay_between_retries_seconds,
    };
    let baseline_sources = args.baseline_sources.clone().unwrap_or_else(|| {
        sources::defaults_for_chain(common.chain_id)
            .expect("failed to get default baseline sources")
    });
    tracing::info!(?baseline_sources, "using baseline sources");
    let pool_caches: HashMap<BaselineSource, Arc<PoolCache>> =
        sources::uniswap_like_liquidity_sources(&common.web3, &baseline_sources)
            .await
            .expect("failed to load baseline source uniswap liquidity")
            .into_iter()
            .map(|(source, (_, pool_fetcher))| {
                let pool_cache = PoolCache::new(
                    cache_config,
                    pool_fetcher,
                    common.current_block_stream.clone(),
                )
                .expect("failed to create pool cache");
                (source, Arc::new(pool_cache))
            })
            .collect();
    let (balancer_pool_maintainer, balancer_v2_liquidity) =
        if baseline_sources.contains(&BaselineSource::BalancerV2) {
            let factories = args
                .balancer_factories
                .clone()
                .unwrap_or_else(|| BalancerFactoryKind::for_chain(common.chain_id));
            let contracts = BalancerContracts::new(&common.web3, factories)
                .await
                .unwrap();
            let balancer_pool_fetcher = Arc::new(
                BalancerPoolFetcher::new(
                    common.chain_id,
                    common.token_info_fetcher.clone(),
                    cache_config,
                    common.current_block_stream.clone(),
                    common.client.clone(),
                    &contracts,
                    args.balancer_pool_deny_list.clone(),
                )
                .await
                .expect("failed to create Balancer pool fetcher"),
            );
            (
                Some(balancer_pool_fetcher.clone() as Arc<dyn Maintaining>),
                Some(BalancerV2Liquidity::new(
                    common.web3.clone(),
                    balancer_pool_fetcher,
                    base_tokens.clone(),
                    common.settlement_contract.clone(),
                    contracts.vault,
                )),
            )
        } else {
            (None, None)
        };

    let uniswap_like_liquidity = build_amm_artifacts(
        &pool_caches,
        common.settlement_contract.clone(),
        base_tokens.clone(),
        common.web3.clone(),
    )
    .await;

    let zeroex_liquidity = if baseline_sources.contains(&BaselineSource::ZeroEx) {
        let zeroex_api = Arc::new(
            DefaultZeroExApi::new(
                args.zeroex_url
                    .as_deref()
                    .unwrap_or(DefaultZeroExApi::DEFAULT_URL),
                args.zeroex_api_key.clone(),
                common.client.clone(),
            )
            .unwrap(),
        );

        Some(ZeroExLiquidity::new(
            common.web3.clone(),
            zeroex_api,
            contracts::IZeroEx::deployed(&common.web3).await.unwrap(),
            base_tokens.clone(),
            common.settlement_contract.clone(),
        ))
    } else {
        None
    };

    let uniswap_v3_liquidity = if baseline_sources.contains(&BaselineSource::UniswapV3) {
        let uniswap_v3_pool_fetcher = Arc::new(
            UniswapV3PoolFetcher::new(
                common.chain_id,
                args.liquidity_fetcher_max_age_update,
                common.client.clone(),
            )
            .await
            .expect("failed to create UniswapV3 pool fetcher in solver"),
        );

        Some(UniswapV3Liquidity::new(
            UniswapV3SwapRouter::deployed(&common.web3).await.unwrap(),
            common.settlement_contract.clone(),
            base_tokens.clone(),
            common.web3.clone(),
            uniswap_v3_pool_fetcher,
        ))
    } else {
        None
    };

    let maintainer = ServiceMaintenance {
        maintainers: pool_caches
            .into_iter()
            .map(|(_, cache)| cache as Arc<dyn Maintaining>)
            .chain(balancer_pool_maintainer)
            .collect(),
    };
    tokio::task::spawn(
        maintainer.run_maintenance_on_new_block(common.current_block_stream.clone()),
    );

    let liquidity_collector = Box::new(LiquidityCollector {
        uniswap_like_liquidity,
        balancer_v2_liquidity,
        zeroex_liquidity,
        uniswap_v3_liquidity,
    });
    Ok(Arc::new(AuctionConverter::new(
        common.gas_price_estimator.clone(),
        liquidity_collector,
        common.order_converter.clone(),
    )))
}

async fn build_amm_artifacts(
    sources: &HashMap<BaselineSource, Arc<PoolCache>>,
    settlement_contract: contracts::GPv2Settlement,
    base_tokens: Arc<BaseTokens>,
    web3: shared::Web3,
) -> Vec<UniswapLikeLiquidity> {
    let mut res = vec![];
    for (source, pool_cache) in sources {
        let router_address = match source {
            BaselineSource::UniswapV2 => contracts::UniswapV2Router02::deployed(&web3)
                .await
                .expect("couldn't load deployed UniswapV2 router")
                .address(),
            BaselineSource::SushiSwap => contracts::SushiSwapRouter::deployed(&web3)
                .await
                .expect("couldn't load deployed SushiSwap router")
                .address(),
            BaselineSource::Honeyswap => contracts::HoneyswapRouter::deployed(&web3)
                .await
                .expect("couldn't load deployed Honeyswap router")
                .address(),
            BaselineSource::Baoswap => contracts::BaoswapRouter::deployed(&web3)
                .await
                .expect("couldn't load deployed Baoswap router")
                .address(),
            BaselineSource::Swapr => contracts::SwaprRouter::deployed(&web3)
                .await
                .expect("couldn't load deployed Swapr router")
                .address(),
            BaselineSource::BalancerV2 => continue,
            BaselineSource::ZeroEx => continue,
            BaselineSource::UniswapV3 => continue,
        };
        res.push(UniswapLikeLiquidity::new(
            IUniswapLikeRouter::at(&web3, router_address),
            settlement_contract.clone(),
            base_tokens.clone(),
            web3.clone(),
            pool_cache.clone(),
        ));
    }
    res
}

async fn build_drivers(common: &CommonComponents, args: &Arguments) -> Vec<(Arc<Driver>, String)> {
    let solvers = build_solvers(common, args).await;
    let submitter = build_submitter(common, args).await;
    let settlement_rater = Arc::new(SettlementRater {
        access_list_estimator: common.access_list_estimator.clone(),
        settlement_contract: common.settlement_contract.clone(),
        web3: common.web3.clone(),
    });
    let auction_converter = build_auction_converter(common, args).await.unwrap();
    let tenderly = args
        .tenderly_url
        .clone()
        .zip(args.tenderly_api_key.clone())
        .and_then(|(url, api_key)| TenderlyApi::new(url, common.client.clone(), &api_key).ok());
    let metrics = Arc::new(Metrics::new().unwrap());

    let settlement_ranker = Arc::new(SettlementRanker {
        metrics: metrics.clone(),
        settlement_rater: settlement_rater.clone(),
        min_order_age: std::time::Duration::from_secs(30),
        max_settlement_price_deviation: None,
        token_list_restriction_for_price_checks: solver::settlement::PriceCheckTokens::All,
    });
    let logger = Arc::new(DriverLogger {
        web3: common.web3.clone(),
        network_id: common.network_id.clone(),
        metrics,
        settlement_contract: common.settlement_contract.clone(),
        simulation_gas_limit: args.simulation_gas_limit,
        tenderly,
    });

    solvers
        .into_iter()
        .map(|solver| {
            let name = solver.name().to_string();
            let driver = Arc::new(Driver {
                solver: Arc::new(CommitRevealSolver::new(
                    solver,
                    common.gas_price_estimator.clone(),
                    settlement_ranker.clone(),
                    logger.clone(),
                )),
                submitter: submitter.clone(),
                auction_converter: auction_converter.clone(),
                block_stream: common.current_block_stream.clone(),
                logger: logger.clone(),
                settlement_rater: settlement_rater.clone(),
                gas_price_estimator: common.gas_price_estimator.clone(),
            });
            (driver, name)
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let args = driver::arguments::Arguments::parse();
    shared::tracing::initialize(args.log_filter.as_str(), args.log_stderr_threshold);
    tracing::info!("running driver with validated arguments:\n{}", args);
    global_metrics::setup_metrics_registry(Some("gp_v2_driver".into()), None);
    let common = init_common_components(&args).await;

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve_api = serve_api(
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
        build_drivers(&common, &args).await,
    );

    futures::pin_mut!(serve_api);
    tokio::select! {
        result = &mut serve_api => tracing::error!(?result, "API task exited"),
        _ = shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            match tokio::time::timeout(Duration::from_secs(10), serve_api).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => tracing::error!("API shutdown exceeded timeout"),
            }
        }
    };
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept main signals for graceful shutdown
    // Kubernetes sends sigterm, whereas locally sigint (ctrl-c) is most common
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await
    };
    let sigint = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .unwrap()
            .recv()
            .await;
    };
    futures::pin_mut!(sigint);
    futures::pin_mut!(sigterm);
    futures::future::select(sigterm, sigint).await;
}

#[cfg(windows)]
async fn shutdown_signal() {
    // We don't support signal handling on windows
    std::future::pending().await
}
