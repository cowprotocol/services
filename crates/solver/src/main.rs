use anyhow::Context;
use clap::{ArgEnum, Parser};
use contracts::{BalancerV2Vault, IUniswapLikeRouter, WETH9};
use num::rational::Ratio;
use primitive_types::U256;
use shared::{
    baseline_solver::BaseTokens,
    current_block::current_block_stream,
    maintenance::{Maintaining, ServiceMaintenance},
    metrics::serve_metrics,
    network::network_name,
    recent_block_cache::CacheConfig,
    sources::{
        self,
        balancer_v2::{pool_fetching::BalancerContracts, BalancerFactoryKind, BalancerPoolFetcher},
        uniswap_v2::pool_cache::PoolCache,
        BaselineSource,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    token_list::TokenList,
    transport::{create_instrumented_transport, http::HttpTransport},
    zeroex_api::DefaultZeroExApi,
};
use solver::{
    arguments::TransactionStrategyArg,
    driver::Driver,
    liquidity::{
        balancer_v2::BalancerV2Liquidity, order_converter::OrderConverter,
        uniswap_v2::UniswapLikeLiquidity, zeroex::ZeroExLiquidity,
    },
    liquidity_collector::LiquidityCollector,
    metrics::Metrics,
    orderbook::OrderBookApi,
    settlement_simulation::TenderlyApi,
    settlement_submission::{
        submitter::{
            custom_nodes_api::CustomNodesApi, eden_api::EdenApi, flashbots_api::FlashbotsApi,
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
    tracing::info!("running solver with validated arguments:\n{}", args);

    global_metrics::setup_metrics_registry(Some("gp_v2_solver".into()), None);
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
                let pool_cache = PoolCache::new(
                    cache_config,
                    pool_fetcher,
                    current_block_stream.clone(),
                    metrics.clone(),
                )
                .expect("failed to create pool cache");
                (source, Arc::new(pool_cache))
            })
            .collect();

    let (balancer_pool_maintainer, balancer_v2_liquidity) =
        if baseline_sources.contains(&BaselineSource::BalancerV2) {
            let contracts = BalancerContracts::new(&web3).await.unwrap();
            let balancer_pool_fetcher = Arc::new(
                BalancerPoolFetcher::new(
                    chain_id,
                    token_info_fetcher.clone(),
                    args.shared
                        .balancer_factories
                        .as_deref()
                        .unwrap_or_else(BalancerFactoryKind::value_variants),
                    cache_config,
                    current_block_stream.clone(),
                    metrics.clone(),
                    client.clone(),
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
        args.paraswap_slippage_bps,
        args.shared.disabled_paraswap_dexs,
        args.shared.paraswap_partner,
        client.clone(),
        metrics.clone(),
        zeroex_api.clone(),
        args.zeroex_slippage_bps,
        args.shared.disabled_zeroex_sources,
        args.oneinch_slippage_bps,
        args.shared.quasimodo_uses_internal_buffers,
        args.shared.mip_uses_internal_buffers,
        args.shared.one_inch_url,
        args.external_solvers.unwrap_or_default(),
        args.oneinch_max_slippage_in_eth
            .map(|float| U256::from_f64_lossy(float * 1e18)),
    )
    .expect("failure creating solvers");

    let zeroex_liquidity = if baseline_sources.contains(&BaselineSource::ZeroEx) {
        Some(ZeroExLiquidity {
            api: zeroex_api,
            zeroex: contracts::IZeroEx::deployed(&web3).await.unwrap(),
            base_tokens,
            gpv2: settlement_contract.clone(),
        })
    } else {
        None
    };

    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity,
        balancer_v2_liquidity,
        zeroex_liquidity,
    };
    let market_makable_token_list =
        TokenList::from_url(&args.market_makable_token_list, chain_id, client.clone())
            .await
            .map_err(|err| tracing::error!("Couldn't fetch market makable token list: {}", err))
            .ok();
    let submission_nodes_with_url = args
        .transaction_submission_nodes
        .into_iter()
        .enumerate()
        .map(|(index, url)| {
            let transport = create_instrumented_transport(
                HttpTransport::new(client.clone(), url.clone(), index.to_string()),
                metrics.clone(),
            );
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
            node_network_id, network_id,
            "network id of custom node doesn't match main node"
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
            TransactionStrategyArg::PublicMempool => {
                transaction_strategies.push(TransactionStrategy::CustomNodes(StrategyArgs {
                    submit_api: Box::new(CustomNodesApi::new(vec![web3.clone()])),
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
                    submit_api: Box::new(CustomNodesApi::new(submission_nodes.clone())),
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
        client.clone(),
        args.shared.solver_competition_auth,
    );
    let order_converter = OrderConverter {
        native_token: native_token_contract.clone(),
        fee_objective_scaling_factor: args.fee_objective_scaling_factor,
    };
    let tenderly = args
        .tenderly_url
        .zip(args.tenderly_api_key)
        .and_then(|(url, api_key)| TenderlyApi::new(url, client.clone(), &api_key).ok());

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
        args.max_merged_settlements,
        args.solver_time_limit,
        market_makable_token_list,
        current_block_stream.clone(),
        solution_submitter,
        args.max_settlements_per_solver,
        api,
        order_converter,
        args.weth_unwrap_factor,
        args.simulation_gas_limit,
        args.fee_objective_scaling_factor,
        args.max_settlement_price_deviation
            .map(|max_price_deviation| Ratio::from_float(max_price_deviation).unwrap()),
        args.token_list_restriction_for_price_checks.into(),
        tenderly,
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
