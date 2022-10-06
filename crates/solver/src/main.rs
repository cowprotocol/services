use anyhow::Context;
use clap::Parser;
use contracts::{BalancerV2Vault, IUniswapLikeRouter, UniswapV3SwapRouter, WETH9};
use num::rational::Ratio;
use shared::{
    baseline_solver::BaseTokens,
    current_block::current_block_stream,
    http_client::HttpClientFactory,
    maintenance::{Maintaining, ServiceMaintenance},
    metrics::serve_metrics,
    network::network_name,
    recent_block_cache::CacheConfig,
    sources::{
        self,
        balancer_v2::{pool_fetching::BalancerContracts, BalancerFactoryKind, BalancerPoolFetcher},
        uniswap_v2::pool_cache::PoolCache,
        uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
        BaselineSource,
    },
    tenderly_api::{TenderlyApi, TenderlyHttpApi},
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    token_list::TokenList,
    zeroex_api::DefaultZeroExApi,
};
use solver::{
    arguments::TransactionStrategyArg,
    driver::Driver,
    liquidity::{
        balancer_v2::BalancerV2Liquidity, order_converter::OrderConverter,
        uniswap_v2::UniswapLikeLiquidity, uniswap_v3::UniswapV3Liquidity, zeroex::ZeroExLiquidity,
    },
    liquidity_collector::LiquidityCollector,
    metrics::Metrics,
    orderbook::OrderBookApi,
    settlement_submission::{
        submitter::{
            eden_api::EdenApi, flashbots_api::FlashbotsApi, public_mempool_api::PublicMempoolApi,
            Strategy,
        },
        GlobalTxPool, SolutionSubmitter, StrategyArgs, TransactionStrategy,
    },
};
use std::{collections::HashMap, sync::Arc};

#[tokio::main]
async fn main() {
    let args = solver::arguments::Arguments::parse();
    shared::tracing::initialize(
        args.shared.log_filter.as_str(),
        args.shared.log_stderr_threshold,
    );
    shared::exit_process_on_panic::set_panic_hook();
    tracing::info!("running solver with validated arguments:\n{}", args);

    global_metrics::setup_metrics_registry(Some("gp_v2_solver".into()), None);
    let metrics = Arc::new(Metrics::new().expect("Couldn't register metrics"));

    let http_factory = HttpClientFactory::new(&args.http_client);

    let web3 = shared::web3(&http_factory, &args.shared.node_url, "base");
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
    let vault_contract = BalancerV2Vault::deployed(&web3).await.ok();
    let native_token_contract = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let base_tokens = Arc::new(BaseTokens::new(
        native_token_contract.address(),
        &args.shared.base_tokens,
    ));

    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            &http_factory,
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
        maximum_recent_block_age: args.shared.pool_cache_maximum_recent_block_age,
        max_retries: args.shared.pool_cache_maximum_retries,
        delay_between_retries: args.shared.pool_cache_delay_between_retries_seconds,
        ..Default::default()
    };
    let baseline_sources = args.shared.baseline_sources.unwrap_or_else(|| {
        sources::defaults_for_chain(chain_id).expect("failed to get default baseline sources")
    });
    tracing::info!(?baseline_sources, "using baseline sources");
    let pool_caches: HashMap<BaselineSource, Arc<PoolCache>> =
        sources::uniswap_like_liquidity_sources(&web3, &baseline_sources)
            .await
            .expect("failed to load baseline source uniswap liquidity")
            .into_iter()
            .map(|(source, (_, pool_fetcher))| {
                let pool_cache =
                    PoolCache::new(cache_config, pool_fetcher, current_block_stream.clone())
                        .expect("failed to create pool cache");
                (source, Arc::new(pool_cache))
            })
            .collect();

    let (balancer_pool_maintainer, balancer_v2_liquidity) =
        if baseline_sources.contains(&BaselineSource::BalancerV2) {
            let factories = args
                .shared
                .balancer_factories
                .unwrap_or_else(|| BalancerFactoryKind::for_chain(chain_id));
            let contracts = BalancerContracts::new(&web3, factories).await.unwrap();
            let balancer_pool_fetcher = Arc::new(
                BalancerPoolFetcher::new(
                    chain_id,
                    token_info_fetcher.clone(),
                    cache_config,
                    current_block_stream.clone(),
                    http_factory.create(),
                    web3.clone(),
                    &contracts,
                    args.shared.balancer_pool_deny_list,
                )
                .await
                .expect("failed to create Balancer pool fetcher"),
            );
            (
                Some(balancer_pool_fetcher.clone() as Arc<dyn Maintaining>),
                Some(BalancerV2Liquidity::new(
                    web3.clone(),
                    balancer_pool_fetcher,
                    base_tokens.clone(),
                    settlement_contract.clone(),
                    contracts.vault,
                )),
            )
        } else {
            (None, None)
        };

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
            &http_factory,
            args.shared
                .zeroex_url
                .as_deref()
                .unwrap_or(DefaultZeroExApi::DEFAULT_URL),
            args.shared.zeroex_api_key,
        )
        .unwrap(),
    );

    let order_converter = Arc::new(OrderConverter {
        native_token: native_token_contract.clone(),
        fee_objective_scaling_factor: args.fee_objective_scaling_factor,
    });

    let solver = solver::solver::create(
        web3.clone(),
        solvers,
        base_tokens.clone(),
        native_token_contract.address(),
        args.mip_solver_url,
        args.cow_dex_ag_solver_url,
        args.quasimodo_solver_url,
        args.balancer_sor_url,
        &settlement_contract,
        vault_contract.as_ref(),
        token_info_fetcher,
        network_name.to_string(),
        chain_id,
        args.shared.disabled_one_inch_protocols,
        args.shared.disabled_paraswap_dexs,
        args.shared.paraswap_partner,
        &http_factory,
        metrics.clone(),
        zeroex_api.clone(),
        args.shared.disabled_zeroex_sources,
        args.shared.quasimodo_uses_internal_buffers,
        args.shared.mip_uses_internal_buffers,
        args.shared.one_inch_url,
        args.shared.one_inch_referrer_address,
        args.external_solvers.unwrap_or_default(),
        order_converter.clone(),
        args.max_settlements_per_solver,
        args.max_merged_settlements,
        &args.slippage,
    )
    .expect("failure creating solvers");

    metrics.initialize_solver_metrics(
        &solver
            .iter()
            .map(|solver| solver.name())
            .collect::<Vec<_>>(),
    );

    let zeroex_liquidity = if baseline_sources.contains(&BaselineSource::ZeroEx) {
        Some(ZeroExLiquidity::new(
            web3.clone(),
            zeroex_api,
            contracts::IZeroEx::deployed(&web3).await.unwrap(),
            base_tokens.clone(),
            settlement_contract.clone(),
        ))
    } else {
        None
    };

    let (uniswap_v3_liquidity, uniswap_v3_maintainer) =
        if baseline_sources.contains(&BaselineSource::UniswapV3) {
            let uniswap_v3_pool_fetcher = Arc::new(
                UniswapV3PoolFetcher::new(
                    chain_id,
                    args.shared.liquidity_fetcher_max_age_update,
                    http_factory.create(),
                )
                .await
                .expect("failed to create UniswapV3 pool fetcher in solver"),
            );

            (
                Some(UniswapV3Liquidity::new(
                    UniswapV3SwapRouter::deployed(&web3).await.unwrap(),
                    settlement_contract.clone(),
                    base_tokens.clone(),
                    web3.clone(),
                    uniswap_v3_pool_fetcher.clone(),
                )),
                Some(uniswap_v3_pool_fetcher.clone() as Arc<dyn Maintaining>),
            )
        } else {
            (None, None)
        };

    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity,
        balancer_v2_liquidity,
        zeroex_liquidity,
        uniswap_v3_liquidity,
    };
    let market_makable_token_list = TokenList::from_url(
        &args.market_makable_token_list,
        chain_id,
        http_factory.create(),
    )
    .await
    .map_err(|err| tracing::error!("Couldn't fetch market makable token list: {}", err))
    .ok();
    let submission_nodes_with_url = args
        .transaction_submission_nodes
        .into_iter()
        .enumerate()
        .map(|(index, url)| (shared::web3(&http_factory, &url, index), url))
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
            node_network_id, network_id,
            "network id of submission node doesn't match main node"
        );
    }
    let submission_nodes = submission_nodes_with_url
        .into_iter()
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    let submitted_transactions = GlobalTxPool::default();
    let mut transaction_strategies = vec![];
    for strategy in args.transaction_strategy {
        match strategy {
            TransactionStrategyArg::Eden => {
                transaction_strategies.push(TransactionStrategy::Eden(StrategyArgs {
                    submit_api: Box::new(
                        EdenApi::new(http_factory.create(), args.eden_api_url.clone()).unwrap(),
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
                            FlashbotsApi::new(http_factory.create(), flashbots_url).unwrap(),
                        ),
                        max_additional_tip: args.max_additional_flashbot_tip,
                        additional_tip_percentage_of_max_fee: args.additional_tip_percentage,
                        sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::Flashbots),
                    }))
                }
            }
            TransactionStrategyArg::PublicMempool => {
                assert!(
                    !submission_nodes.is_empty(),
                    "missing transaction submission nodes"
                );
                transaction_strategies.push(TransactionStrategy::PublicMempool(StrategyArgs {
                    submit_api: Box::new(PublicMempoolApi::new(
                        submission_nodes.clone(),
                        args.disable_high_risk_public_mempool_transactions,
                    )),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::PublicMempool),
                }))
            }
            TransactionStrategyArg::DryRun => {
                transaction_strategies.push(TransactionStrategy::DryRun)
            }
        }
    }
    let tenderly_api = Some(()).and_then(|_| {
        Some(Arc::new(
            TenderlyHttpApi::new(
                &http_factory,
                args.tenderly_user.as_deref()?,
                args.tenderly_project.as_deref()?,
                args.tenderly_api_key.as_deref()?,
            )
            .expect("failed to create Tenderly API"),
        ) as Arc<dyn TenderlyApi>)
    });
    let access_list_estimator = Arc::new(
        solver::settlement_access_list::create_priority_estimator(
            &web3,
            args.access_list_estimators.as_slice(),
            tenderly_api.clone(),
            network_id.clone(),
        )
        .expect("failed to create access list estimator"),
    );
    let solution_submitter = SolutionSubmitter {
        web3: web3.clone(),
        contract: settlement_contract.clone(),
        gas_price_estimator: gas_price_estimator.clone(),
        target_confirm_time: args.target_confirm_time,
        max_confirm_time: args.max_submission_seconds,
        retry_interval: args.submission_retry_interval_seconds,
        gas_price_cap: args.gas_price_cap,
        transaction_strategies,
        access_list_estimator,
    };
    let api = OrderBookApi::new(
        args.orderbook_url,
        http_factory.create(),
        args.shared.solver_competition_auth.clone(),
    );

    let mut driver = Driver::new(
        settlement_contract,
        liquidity_collector,
        solver,
        gas_price_estimator,
        args.settle_interval,
        native_token_contract.address(),
        args.min_order_age,
        metrics.clone(),
        web3,
        network_id,
        args.solver_time_limit,
        market_makable_token_list,
        current_block_stream.clone(),
        solution_submitter,
        api,
        order_converter,
        args.weth_unwrap_factor,
        args.simulation_gas_limit,
        args.fee_objective_scaling_factor,
        args.max_settlement_price_deviation
            .map(|max_price_deviation| Ratio::from_float(max_price_deviation).unwrap()),
        args.token_list_restriction_for_price_checks.into(),
        tenderly_api,
        args.solution_comparison_decimal_cutoff,
    );

    let maintainer = ServiceMaintenance {
        maintainers: pool_caches
            .into_iter()
            .map(|(_, cache)| cache as Arc<dyn Maintaining>)
            .chain(balancer_pool_maintainer)
            .chain(uniswap_v3_maintainer)
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
