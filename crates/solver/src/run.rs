use {
    crate::{
        arguments::{Arguments, TransactionStrategyArg},
        driver::Driver,
        liquidity::{
            balancer_v2::BalancerV2Liquidity,
            order_converter::OrderConverter,
            uniswap_v2::UniswapLikeLiquidity,
            uniswap_v3::UniswapV3Liquidity,
            zeroex::ZeroExLiquidity,
        },
        liquidity_collector::{LiquidityCollecting, LiquidityCollector},
        metrics::Metrics,
        orderbook::OrderBookApi,
        s3_instance_upload::S3InstanceUploader,
        settlement_post_processing::PostProcessingPipeline,
        settlement_submission::{
            gelato::GelatoSubmitter,
            submitter::{
                eden_api::EdenApi,
                flashbots_api::FlashbotsApi,
                public_mempool_api::{
                    validate_submission_node,
                    PublicMempoolApi,
                    SubmissionNode,
                    SubmissionNodeKind,
                },
                Strategy,
            },
            GlobalTxPool,
            SolutionSubmitter,
            StrategyArgs,
            TransactionStrategy,
        },
    },
    contracts::{BalancerV2Vault, IUniswapLikeRouter, UniswapV3SwapRouter, WETH9},
    ethcontract::errors::DeployError,
    futures::{future, StreamExt},
    model::DomainSeparator,
    num::rational::Ratio,
    shared::{
        baseline_solver::BaseTokens,
        code_fetching::CachedCodeFetcher,
        ethrpc,
        gelato_api::GelatoClient,
        http_client::HttpClientFactory,
        maintenance::{Maintaining, ServiceMaintenance},
        metrics::serve_metrics,
        network::network_name,
        recent_block_cache::CacheConfig,
        sources::{
            self,
            balancer_v2::{
                pool_fetching::BalancerContracts,
                BalancerFactoryKind,
                BalancerPoolFetcher,
            },
            uniswap_v2::{pool_cache::PoolCache, UniV2BaselineSourceParameters},
            uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
            BaselineSource,
        },
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
        token_list::{AutoUpdatingTokenList, TokenListConfiguration},
        zeroex_api::DefaultZeroExApi,
    },
    std::sync::Arc,
};

pub async fn run(args: Arguments) {
    let metrics = Arc::new(Metrics::new().expect("Couldn't register metrics"));

    let http_factory = HttpClientFactory::new(&args.http_client);

    let web3 = ethrpc::web3(
        &args.shared.ethrpc,
        &http_factory,
        &args.shared.node_url,
        "base",
    );

    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    if let Some(expected_chain_id) = args.shared.chain_id {
        assert_eq!(
            chain_id, expected_chain_id,
            "connected to node with incorrect chain ID",
        );
    }

    let network_id = web3
        .net()
        .version()
        .await
        .expect("failed to get network id");
    let network_name = network_name(&network_id, chain_id);
    let settlement_contract = match args.shared.settlement_contract_address {
        Some(address) => contracts::GPv2Settlement::with_deployment_info(&web3, address, None),
        None => contracts::GPv2Settlement::deployed(&web3)
            .await
            .expect("load settlement contract"),
    };
    let vault_contract = match args.shared.balancer_v2_vault_address {
        Some(address) => Some(contracts::BalancerV2Vault::with_deployment_info(
            &web3, address, None,
        )),
        None => match BalancerV2Vault::deployed(&web3).await {
            Ok(contract) => Some(contract),
            Err(DeployError::NotFound(_)) => None,
            Err(err) => panic!("failed to get balancer vault contract: {err}"),
        },
    };
    let native_token = match args.shared.native_token_address {
        Some(address) => contracts::WETH9::with_deployment_info(&web3, address, None),
        None => WETH9::deployed(&web3)
            .await
            .expect("load native token contract"),
    };
    let base_tokens = Arc::new(BaseTokens::new(
        native_token.address(),
        &args.shared.base_tokens,
    ));

    let block_retriever = args.shared.current_block.retriever(web3.clone());
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

    let current_block_stream = args
        .shared
        .current_block
        .stream(web3.clone())
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

    let mut liquidity_sources: Vec<Box<dyn LiquidityCollecting>> = vec![];
    let mut maintainers: Vec<Arc<dyn Maintaining>> = vec![];

    tracing::info!(?baseline_sources, "using baseline sources");
    let univ2_sources = baseline_sources
        .iter()
        .filter_map(|source: &BaselineSource| {
            UniV2BaselineSourceParameters::from_baseline_source(*source, &network_id)
        })
        .chain(args.shared.custom_univ2_baseline_sources.iter().copied());
    let univ2_sources: Vec<(IUniswapLikeRouter, Arc<PoolCache>)> =
        futures::stream::iter(univ2_sources)
            .then(|source: UniV2BaselineSourceParameters| {
                let web3 = &web3;
                let block_stream = &current_block_stream;
                async move {
                    let source = source.into_source(web3).await.unwrap();
                    let cache = Arc::new(
                        PoolCache::new(cache_config, source.pool_fetching, block_stream.clone())
                            .unwrap(),
                    );
                    (source.router, cache)
                }
            })
            .collect()
            .await;
    maintainers.extend(
        univ2_sources
            .iter()
            .map(|(_, cache)| cache.clone() as Arc<_>),
    );

    if baseline_sources.contains(&BaselineSource::BalancerV2) {
        let factories = args
            .shared
            .balancer_factories
            .unwrap_or_else(|| BalancerFactoryKind::for_chain(chain_id));
        let contracts = BalancerContracts::new(&web3, factories).await.unwrap();
        let balancer_pool_fetcher = Arc::new(
            BalancerPoolFetcher::new(
                chain_id,
                block_retriever.clone(),
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
        maintainers.push(balancer_pool_fetcher.clone());
        liquidity_sources.push(Box::new(BalancerV2Liquidity::new(
            web3.clone(),
            balancer_pool_fetcher,
            settlement_contract.clone(),
            contracts.vault,
        )));
    }

    let uniswap_like_liquidity: Vec<Box<dyn LiquidityCollecting>> = univ2_sources
        .into_iter()
        .map(|(router, cache)| -> Box<dyn LiquidityCollecting> {
            Box::new(UniswapLikeLiquidity::new(
                router,
                settlement_contract.clone(),
                web3.clone(),
                cache,
            ))
        })
        .collect();
    liquidity_sources.extend(uniswap_like_liquidity);

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
        native_token: native_token.clone(),
    });

    let market_makable_token_list_configuration = TokenListConfiguration {
        url: args.market_makable_token_list,
        update_interval: args.market_makable_token_list_update_interval,
        chain_id,
        client: http_factory.create(),
        hardcoded: args.market_makable_tokens.unwrap_or_default(),
    };
    // updated in background task
    let market_makable_token_list =
        AutoUpdatingTokenList::from_configuration(market_makable_token_list_configuration).await;

    let post_processing_pipeline = Arc::new(PostProcessingPipeline::new(
        native_token.address(),
        web3.clone(),
        args.weth_unwrap_factor,
        settlement_contract.clone(),
        market_makable_token_list.clone(),
    ));

    let domain = DomainSeparator::new(chain_id, settlement_contract.address());

    let s3_instance_uploader = args
        .s3_upload
        .into_config()
        .unwrap()
        .map(S3InstanceUploader::new)
        .map(Arc::new);

    let solver = crate::solver::create(
        web3.clone(),
        solvers,
        base_tokens.clone(),
        native_token.address(),
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
        args.shared.use_internal_buffers,
        args.shared.one_inch_url,
        args.shared.one_inch_referrer_address,
        args.external_solvers.unwrap_or_default(),
        order_converter.clone(),
        args.max_settlements_per_solver,
        args.max_merged_settlements,
        &args.slippage,
        market_makable_token_list,
        &args.order_prioritization,
        post_processing_pipeline,
        &domain,
        s3_instance_uploader,
        &args.score_params,
    )
    .expect("failure creating solvers");

    metrics.initialize_solver_metrics(
        &solver
            .iter()
            .map(|solver| solver.name())
            .collect::<Vec<_>>(),
    );

    if baseline_sources.contains(&BaselineSource::ZeroEx) {
        liquidity_sources.push(Box::new(ZeroExLiquidity::new(
            web3.clone(),
            zeroex_api,
            contracts::IZeroEx::deployed(&web3).await.unwrap(),
            settlement_contract.clone(),
        )));
    }

    if baseline_sources.contains(&BaselineSource::UniswapV3) {
        let uniswap_v3_pool_fetcher = Arc::new(
            UniswapV3PoolFetcher::new(
                chain_id,
                web3.clone(),
                http_factory.create(),
                block_retriever,
                args.shared.max_pools_to_initialize_cache,
            )
            .await
            .expect("error innitializing Uniswap V3 pool fetcher"),
        );
        maintainers.push(uniswap_v3_pool_fetcher.clone());
        liquidity_sources.push(Box::new(UniswapV3Liquidity::new(
            UniswapV3SwapRouter::deployed(&web3).await.unwrap(),
            settlement_contract.clone(),
            web3.clone(),
            uniswap_v3_pool_fetcher,
        )));
    }

    let liquidity_collector = LiquidityCollector {
        liquidity_sources,
        base_tokens,
    };
    let submission_nodes = future::join_all(
        args.transaction_submission_nodes
            .into_iter()
            .enumerate()
            .map(|(index, url)| {
                let name = format!("broadcast {index}");
                (name, url, SubmissionNodeKind::Broadcast)
            })
            .chain(
                args.transaction_notification_nodes
                    .into_iter()
                    .enumerate()
                    .map(|(index, url)| {
                        let name = format!("notify {index}");
                        (name, url, SubmissionNodeKind::Notification)
                    }),
            )
            .map(|(name, url, kind)| {
                let web3 = ethrpc::web3(&args.shared.ethrpc, &http_factory, &url, name);
                let expected_network_id = &network_id;
                async move {
                    if let Err(err) = validate_submission_node(&web3, expected_network_id).await {
                        tracing::error!("Error validating {kind:?} submission node {url}: {err}");
                        assert!(kind == SubmissionNodeKind::Notification);
                    }
                    SubmissionNode::new(kind, web3)
                }
            }),
    )
    .await;
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
                    submission_nodes.iter().any(|node| node.can_broadcast()),
                    "At least one submission node that can broadcast transactions must be \
                     available"
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
            TransactionStrategyArg::Gelato => {
                transaction_strategies.push(TransactionStrategy::Gelato(Arc::new(
                    GelatoSubmitter::new(
                        web3.clone(),
                        settlement_contract.clone(),
                        GelatoClient::new(&http_factory, args.gelato_api_key.clone().unwrap()),
                        args.gelato_submission_poll_interval,
                    )
                    .await
                    .unwrap(),
                )))
            }
            TransactionStrategyArg::DryRun => {
                transaction_strategies.push(TransactionStrategy::DryRun)
            }
        }
    }
    let access_list_estimator = Arc::new(
        crate::settlement_access_list::create_priority_estimator(
            &web3,
            args.access_list_estimators.as_slice(),
            args.shared
                .tenderly
                .get_api_instance(&http_factory, "access_lists".to_owned())
                .expect("failed to create Tenderly API"),
            network_id.clone(),
        )
        .expect("failed to create access list estimator"),
    );
    let code_fetcher = Arc::new(CachedCodeFetcher::new(Arc::new(web3.clone())));
    let solution_submitter = SolutionSubmitter {
        web3: web3.clone(),
        contract: settlement_contract.clone(),
        gas_price_estimator: gas_price_estimator.clone(),
        target_confirm_time: args.target_confirm_time,
        max_confirm_time: args.max_submission_seconds,
        retry_interval: args.submission_retry_interval_seconds,
        transaction_strategies,
        access_list_estimator,
        code_fetcher: code_fetcher.clone(),
    };
    let api = OrderBookApi::new(
        args.orderbook_url,
        http_factory.create(),
        args.shared.solver_competition_auth.clone(),
    );
    let network_time_between_blocks = args
        .shared
        .network_block_interval
        .or_else(|| shared::network::block_interval(&network_id, chain_id))
        .expect("unknown network block interval");

    let mut driver = Driver::new(
        settlement_contract,
        liquidity_collector,
        solver,
        gas_price_estimator,
        args.gas_price_cap,
        args.settle_interval,
        native_token.address(),
        metrics.clone(),
        web3,
        network_id,
        args.solver_time_limit,
        network_time_between_blocks,
        args.additional_mining_deadline,
        args.skip_non_positive_score_settlements,
        current_block_stream.clone(),
        solution_submitter,
        api,
        order_converter,
        args.simulation_gas_limit,
        args.max_settlement_price_deviation
            .map(|max_price_deviation| Ratio::from_float(max_price_deviation).unwrap()),
        args.token_list_restriction_for_price_checks.into(),
        args.shared
            .tenderly
            .get_api_instance(&http_factory, "driver".to_owned())
            .expect("failed to create Tenderly API"),
        args.solution_comparison_decimal_cutoff,
        code_fetcher,
        args.auction_rewards_activation_timestamp,
    );

    let maintainer = ServiceMaintenance::new(maintainers);
    tokio::task::spawn(maintainer.run_maintenance_on_new_block(current_block_stream));

    serve_metrics(metrics, ([0, 0, 0, 0], args.metrics_port).into());
    driver.run_forever().await
}
