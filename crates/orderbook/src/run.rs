use {
    crate::{
        arguments::Arguments,
        database::Postgres,
        orderbook::Orderbook,
        serve_api,
        verify_deployed_contract_constants,
    },
    contracts::{
        BalancerV2Vault,
        CowProtocolToken,
        CowProtocolVirtualToken,
        IUniswapV3Factory,
        WETH9,
    },
    ethcontract::errors::DeployError,
    futures::StreamExt,
    model::{order::BUY_ETH_ADDRESS, DomainSeparator},
    shared::{
        account_balances::Web3BalanceFetcher,
        bad_token::{
            cache::CachingDetector,
            instrumented::InstrumentedBadTokenDetectorExt,
            list_based::{ListBasedDetector, UnknownTokenStrategy},
            token_owner_finder,
            trace_call::TraceCallDetector,
        },
        baseline_solver::BaseTokens,
        code_fetching::CachedCodeFetcher,
        fee_subsidy::{
            config::FeeSubsidyConfiguration,
            cow_token::CowSubsidy,
            FeeSubsidies,
            FeeSubsidizing,
        },
        gas_price::InstrumentedGasEstimator,
        http_client::HttpClientFactory,
        maintenance::{Maintaining, ServiceMaintenance},
        metrics::{serve_metrics, DEFAULT_METRICS_PORT},
        network::network_name,
        oneinch_api::OneInchClientImpl,
        order_quoting::{OrderQuoter, QuoteHandler},
        order_validation::{OrderValidPeriodConfiguration, OrderValidator, SignatureConfiguration},
        price_estimation::{
            factory::{self, PriceEstimatorFactory},
            PriceEstimating,
        },
        recent_block_cache::CacheConfig,
        signature_validator::Web3SignatureValidator,
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
            PoolAggregator,
        },
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
        zeroex_api::DefaultZeroExApi,
    },
    std::{sync::Arc, time::Duration},
    tokio::task,
};

pub async fn run(args: Arguments) {
    let http_factory = HttpClientFactory::new(&args.http_client);

    let web3 = shared::ethrpc::web3(
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

    let settlement_contract = match args.shared.settlement_contract_address {
        Some(address) => contracts::GPv2Settlement::with_deployment_info(&web3, address, None),
        None => contracts::GPv2Settlement::deployed(&web3)
            .await
            .expect("load settlement contract"),
    };
    let vault_relayer = settlement_contract
        .vault_relayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");
    let native_token = match args.shared.native_token_address {
        Some(address) => contracts::WETH9::with_deployment_info(&web3, address, None),
        None => WETH9::deployed(&web3)
            .await
            .expect("load native token contract"),
    };

    let network = web3
        .net()
        .version()
        .await
        .expect("Failed to retrieve network version ID");
    let network_name = network_name(&network, chain_id);

    let signature_validator = Arc::new(Web3SignatureValidator::new(web3.clone()));

    let vault = match args.shared.balancer_v2_vault_address {
        Some(address) => Some(contracts::BalancerV2Vault::with_deployment_info(
            &web3, address, None,
        )),
        None => match BalancerV2Vault::deployed(&web3).await {
            Ok(contract) => Some(contract),
            Err(DeployError::NotFound(_)) => {
                tracing::warn!("balancer contracts are not deployed on this network");
                None
            }
            Err(err) => panic!("failed to get balancer vault contract: {err}"),
        },
    };

    verify_deployed_contract_constants(&settlement_contract, chain_id)
        .await
        .expect("Deployed contract constants don't match the ones in this binary");
    let domain_separator = DomainSeparator::new(chain_id, settlement_contract.address());
    let postgres = Postgres::new(args.db_url.as_str()).expect("failed to create database");

    let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
        web3.clone(),
        vault.clone(),
        vault_relayer,
        settlement_contract.address(),
    ));

    let gas_price_estimator = Arc::new(InstrumentedGasEstimator::new(
        shared::gas_price_estimation::create_priority_estimator(
            &http_factory,
            &web3,
            args.shared.gas_estimators.as_slice(),
            args.shared.blocknative_api_key.clone(),
        )
        .await
        .expect("failed to create gas price estimator"),
    ));

    let baseline_sources = args.shared.baseline_sources.clone().unwrap_or_else(|| {
        sources::defaults_for_chain(chain_id).expect("failed to get default baseline sources")
    });
    tracing::info!(?baseline_sources, "using baseline sources");
    let univ2_sources = baseline_sources
        .iter()
        .filter_map(|source: &BaselineSource| {
            UniV2BaselineSourceParameters::from_baseline_source(*source, &network)
        })
        .chain(args.shared.custom_univ2_baseline_sources.iter().copied());
    let (pair_providers, pool_fetchers): (Vec<_>, Vec<_>) = futures::stream::iter(univ2_sources)
        .then(|source: UniV2BaselineSourceParameters| {
            let web3 = &web3;
            async move {
                let source = source.into_source(web3).await.unwrap();
                (source.pair_provider, source.pool_fetching)
            }
        })
        .unzip()
        .await;

    let base_tokens = Arc::new(BaseTokens::new(
        native_token.address(),
        &args.shared.base_tokens,
    ));
    let mut allowed_tokens = args.allowed_tokens.clone();
    allowed_tokens.extend(base_tokens.tokens().iter().copied());
    allowed_tokens.push(BUY_ETH_ADDRESS);
    let unsupported_tokens = args.unsupported_tokens.clone();

    let uniswapv3_factory = match IUniswapV3Factory::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };

    let finder = token_owner_finder::init(
        &args.token_owner_finder,
        web3.clone(),
        chain_id,
        &http_factory,
        &pair_providers,
        vault.as_ref(),
        uniswapv3_factory.as_ref(),
        &base_tokens,
    )
    .await
    .expect("failed to initialize token owner finders");

    let trace_call_detector = args.tracing_node_url.as_ref().map(|tracing_node_url| {
        Box::new(CachingDetector::new(
            Box::new(TraceCallDetector {
                web3: shared::ethrpc::web3(
                    &args.shared.ethrpc,
                    &http_factory,
                    tracing_node_url,
                    "trace",
                ),
                finder,
                settlement_contract: settlement_contract.address(),
            }),
            args.token_quality_cache_expiry,
        ))
    });
    let bad_token_detector = Arc::new(
        ListBasedDetector::new(
            allowed_tokens,
            unsupported_tokens,
            trace_call_detector
                .map(|detector| UnknownTokenStrategy::Forward(detector))
                .unwrap_or(UnknownTokenStrategy::Allow),
        )
        .instrumented(),
    );

    let current_block_stream = args
        .shared
        .current_block
        .stream(web3.clone())
        .await
        .unwrap();

    let pool_aggregator = PoolAggregator { pool_fetchers };

    let cache_config = CacheConfig {
        number_of_blocks_to_cache: args.shared.pool_cache_blocks,
        number_of_entries_to_auto_update: args.pool_cache_lru_size,
        maximum_recent_block_age: args.shared.pool_cache_maximum_recent_block_age,
        max_retries: args.shared.pool_cache_maximum_retries,
        delay_between_retries: args.shared.pool_cache_delay_between_retries_seconds,
    };
    let pool_fetcher = Arc::new(
        PoolCache::new(
            cache_config,
            Arc::new(pool_aggregator),
            current_block_stream.clone(),
        )
        .expect("failed to create pool cache"),
    );
    let block_retriver = args.shared.current_block.retriever(web3.clone());
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let balancer_pool_fetcher = if baseline_sources.contains(&BaselineSource::BalancerV2) {
        let factories = args
            .shared
            .balancer_factories
            .clone()
            .unwrap_or_else(|| BalancerFactoryKind::for_chain(chain_id));
        let contracts = BalancerContracts::new(&web3, factories).await.unwrap();
        let balancer_pool_fetcher = Arc::new(
            BalancerPoolFetcher::new(
                chain_id,
                block_retriver.clone(),
                token_info_fetcher.clone(),
                cache_config,
                current_block_stream.clone(),
                http_factory.create(),
                web3.clone(),
                &contracts,
                args.shared.balancer_pool_deny_list.clone(),
            )
            .await
            .expect("failed to create Balancer pool fetcher"),
        );
        Some(balancer_pool_fetcher)
    } else {
        None
    };
    let uniswap_v3_pool_fetcher = if baseline_sources.contains(&BaselineSource::UniswapV3) {
        Some(Arc::new(
            UniswapV3PoolFetcher::new(
                chain_id,
                web3.clone(),
                http_factory.create(),
                block_retriver,
                args.shared.max_pools_to_initialize_cache,
            )
            .await
            .expect("error innitializing Uniswap V3 pool fetcher"),
        ))
    } else {
        None
    };
    let zeroex_api = Arc::new(
        DefaultZeroExApi::new(
            &http_factory,
            args.shared
                .zeroex_url
                .as_deref()
                .unwrap_or(DefaultZeroExApi::DEFAULT_URL),
            args.shared.zeroex_api_key.clone(),
        )
        .unwrap(),
    );
    let one_inch_api = OneInchClientImpl::new(
        args.shared.one_inch_url.clone(),
        http_factory.create(),
        chain_id,
    )
    .map(Arc::new);

    let simulation_web3 = args.simulation_node_url.as_ref().map(|node_url| {
        shared::ethrpc::web3(&args.shared.ethrpc, &http_factory, node_url, "simulation")
    });

    let mut price_estimator_factory = PriceEstimatorFactory::new(
        &args.price_estimation,
        &args.shared,
        factory::Network {
            web3: web3.clone(),
            simulation_web3,
            name: network_name.to_string(),
            chain_id,
            native_token: native_token.address(),
            settlement: settlement_contract.address(),
            authenticator: settlement_contract
                .authenticator()
                .call()
                .await
                .expect("failed to query solver authenticator address"),
            base_tokens: base_tokens.clone(),
        },
        factory::Components {
            http_factory: http_factory.clone(),
            bad_token_detector: bad_token_detector.clone(),
            uniswap_v2_pools: pool_fetcher.clone(),
            balancer_pools: balancer_pool_fetcher.clone().map(|a| a as _),
            uniswap_v3_pools: uniswap_v3_pool_fetcher.clone().map(|a| a as _),
            tokens: token_info_fetcher.clone(),
            gas_price: gas_price_estimator.clone(),
            zeroex: zeroex_api.clone(),
            oneinch: one_inch_api.ok().map(|a| a as _),
        },
    )
    .expect("failed to initialize price estimator factory");

    let price_estimator = price_estimator_factory
        .price_estimator(
            &args.order_quoting.price_estimators,
            &args.order_quoting.price_estimation_drivers,
        )
        .unwrap();
    let fast_price_estimator = price_estimator_factory
        .fast_price_estimator(
            &args.order_quoting.price_estimators,
            args.fast_price_estimation_results_required,
            &args.order_quoting.price_estimation_drivers,
        )
        .unwrap();
    let native_price_estimator = price_estimator_factory
        .native_price_estimator(
            &args.native_price_estimators,
            &args.order_quoting.price_estimation_drivers,
        )
        .unwrap();

    let cow_token = match CowProtocolToken::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };
    let cow_vtoken = match CowProtocolVirtualToken::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };
    let cow_tokens = match (cow_token, cow_vtoken) {
        (None, None) => None,
        (Some(token), Some(vtoken)) => Some((token, vtoken)),
        _ => panic!("should either have both cow token contracts or none"),
    };
    let cow_subsidy = cow_tokens.map(|(token, vtoken)| {
        tracing::debug!("using cow token contracts for subsidy");
        CowSubsidy::new(
            token,
            vtoken,
            args.order_quoting.cow_fee_factors.unwrap_or_default(),
        )
    });

    let fee_subsidy_config = Arc::new(FeeSubsidyConfiguration {
        fee_discount: args.order_quoting.fee_discount,
        min_discounted_fee: args.order_quoting.min_discounted_fee,
        fee_factor: args.order_quoting.fee_factor,
        liquidity_order_owners: args
            .order_quoting
            .liquidity_order_owners
            .iter()
            .copied()
            .collect(),
    }) as Arc<dyn FeeSubsidizing>;

    let fee_subsidy = match cow_subsidy {
        Some(cow_subsidy) => Arc::new(FeeSubsidies(vec![
            fee_subsidy_config,
            Arc::new(cow_subsidy),
        ])),
        None => fee_subsidy_config,
    };

    let validity_configuration = OrderValidPeriodConfiguration {
        min: args.min_order_validity_period,
        max_market: args.max_order_validity_period,
        max_limit: args.max_limit_order_validity_period,
    };
    let signature_configuration = SignatureConfiguration {
        eip1271: args.enable_eip1271_orders,
        eip1271_skip_creation_validation: args.eip1271_skip_creation_validation,
        presign: args.enable_presign_orders,
    };

    let create_quoter = |price_estimator: Arc<dyn PriceEstimating>| {
        Arc::new(OrderQuoter::new(
            price_estimator,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
            fee_subsidy.clone(),
            Arc::new(postgres.clone()),
            chrono::Duration::from_std(args.order_quoting.eip1271_onchain_quote_validity_seconds)
                .unwrap(),
            chrono::Duration::from_std(args.order_quoting.presign_onchain_quote_validity_seconds)
                .unwrap(),
        ))
    };
    let optimal_quoter = create_quoter(price_estimator.clone());
    let fast_quoter = create_quoter(fast_price_estimator.clone());

    let order_validator = Arc::new(
        OrderValidator::new(
            native_token.clone(),
            args.banned_users.iter().copied().collect(),
            args.order_quoting
                .liquidity_order_owners
                .iter()
                .copied()
                .collect(),
            validity_configuration,
            signature_configuration,
            bad_token_detector.clone(),
            optimal_quoter.clone(),
            balance_fetcher,
            signature_validator,
            Arc::new(postgres.clone()),
            args.max_limit_orders_per_user,
            Arc::new(CachedCodeFetcher::new(Arc::new(web3.clone()))),
            shared::app_data::Validator { size_limit: 1_000 },
        )
        .with_fill_or_kill_limit_orders(args.allow_placing_fill_or_kill_limit_orders)
        .with_partially_fillable_limit_orders(args.allow_placing_partially_fillable_limit_orders)
        .with_eth_smart_contract_payments(args.enable_eth_smart_contract_payments)
        .with_custom_interactions(args.enable_custom_interactions)
        .with_verified_quotes(args.price_estimation.trade_simulator.is_some()),
    );
    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        settlement_contract.address(),
        postgres.clone(),
        order_validator.clone(),
    ));

    let mut maintainers = vec![pool_fetcher as Arc<dyn Maintaining>];
    if let Some(balancer) = balancer_pool_fetcher {
        maintainers.push(balancer);
    }
    if let Some(uniswap_v3) = uniswap_v3_pool_fetcher {
        maintainers.push(uniswap_v3);
    }

    check_database_connection(orderbook.as_ref()).await;
    let quotes =
        Arc::new(QuoteHandler::new(order_validator, optimal_quoter).with_fast_quoter(fast_quoter));
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve_api = serve_api(
        postgres,
        orderbook.clone(),
        quotes,
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
        args.shared.solver_competition_auth,
        native_price_estimator,
    );

    let service_maintainer = ServiceMaintenance::new(maintainers);
    task::spawn(service_maintainer.run_maintenance_on_new_block(current_block_stream));

    let mut metrics_address = args.bind_address;
    metrics_address.set_port(DEFAULT_METRICS_PORT);
    tracing::info!(%metrics_address, "serving metrics");
    let metrics_task = serve_metrics(orderbook, metrics_address);

    futures::pin_mut!(serve_api);
    tokio::select! {
        result = &mut serve_api => panic!("API task exited {result:?}"),
        result = metrics_task => panic!("metrics task exited {result:?}"),
        _ = shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            match tokio::time::timeout(Duration::from_secs(10), serve_api).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => tracing::error!("API shutdown exceeded timeout"),
            }
            std::process::exit(0);
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

async fn check_database_connection(orderbook: &Orderbook) {
    orderbook
        .get_order(&Default::default())
        .await
        .expect("failed to connect to database");
}
