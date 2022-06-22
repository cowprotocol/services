use clap::{ArgEnum, Parser};
use contracts::{
    BalancerV2Vault, CowProtocolToken, CowProtocolVirtualToken, GPv2Settlement, IUniswapV3Factory,
    WETH9,
};
use ethcontract::errors::DeployError;
use model::{
    order::{OrderUid, BUY_ETH_ADDRESS},
    DomainSeparator,
};
use orderbook::{
    account_balances::Web3BalanceFetcher,
    database::{self, orders::OrderFilter, Postgres},
    event_updater::EventUpdater,
    fee::MinFeeCalculator,
    fee_subsidy::{
        config::FeeSubsidyConfiguration, cow_token::CowSubsidy, FeeSubsidies, FeeSubsidizing,
    },
    gas_price::InstrumentedGasEstimator,
    metrics::Metrics,
    order_quoting::{Forget, OrderQuoter, QuoteHandler, QuoteStoring},
    order_validation::OrderValidator,
    orderbook::Orderbook,
    serve_api,
    solvable_orders::SolvableOrdersCache,
    solver_competition::SolverCompetition,
    verify_deployed_contract_constants,
};
use primitive_types::U256;
use shared::{
    bad_token::{
        cache::CachingDetector,
        instrumented::InstrumentedBadTokenDetectorExt,
        list_based::{ListBasedDetector, UnknownTokenStrategy},
        token_owner_finder::{
            blockscout::BlockscoutTokenOwnerFinder, BalancerVaultFinder, TokenOwnerFinding,
            UniswapLikePairProviderFinder, UniswapV3Finder,
        },
        trace_call::TraceCallDetector,
    },
    baseline_solver::BaseTokens,
    current_block::current_block_stream,
    http_solver::{DefaultHttpSolverApi, Objective, SolverConfig},
    maintenance::ServiceMaintenance,
    metrics::{serve_metrics, setup_metrics_registry, DEFAULT_METRICS_PORT},
    network::network_name,
    oneinch_api::OneInchClientImpl,
    paraswap_api::DefaultParaswapApi,
    price_estimation::{
        baseline::BaselinePriceEstimator,
        competition::{CompetitionPriceEstimator, RacingCompetitionPriceEstimator},
        http::HttpPriceEstimator,
        instrumented::InstrumentedPriceEstimator,
        native::NativePriceEstimator,
        native_price_cache::CachingNativePriceEstimator,
        oneinch::OneInchPriceEstimator,
        paraswap::ParaswapPriceEstimator,
        sanitized::SanitizedPriceEstimator,
        zeroex::ZeroExPriceEstimator,
        PriceEstimating, PriceEstimatorType,
    },
    rate_limiter::RateLimiter,
    recent_block_cache::CacheConfig,
    sources::balancer_v2::BalancerFactoryKind,
    sources::{
        self,
        balancer_v2::{pool_fetching::BalancerContracts, BalancerPoolFetcher},
        uniswap_v2::pool_cache::PoolCache,
        BaselineSource, PoolAggregator,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    transport::{create_instrumented_transport, http::HttpTransport},
    zeroex_api::DefaultZeroExApi,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::task;

pub async fn database_metrics(metrics: Arc<Metrics>, database: Postgres) -> ! {
    loop {
        match database.count_rows_in_tables().await {
            Ok(counts) => {
                for (table, count) in counts {
                    metrics.set_table_row_count(table, count);
                }
            }
            Err(err) => tracing::error!(?err, "failed to update db metrics"),
        };
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

#[tokio::main]
async fn main() {
    let args = orderbook::arguments::Arguments::parse();
    shared::tracing::initialize(
        args.shared.log_filter.as_str(),
        args.shared.log_stderr_threshold,
    );
    tracing::info!("running order book with validated arguments:\n{}", args);

    setup_metrics_registry(Some("gp_v2_api".into()), None);
    let metrics = Arc::new(Metrics::new().unwrap());

    let client = shared::http_client(args.shared.http_timeout);

    let transport = create_instrumented_transport(
        HttpTransport::new(client.clone(), args.shared.node_url.clone(), "".to_string()),
        metrics.clone(),
    );
    let web3 = web3::Web3::new(transport);
    let current_block = web3
        .eth()
        .block_number()
        .await
        .expect("block_number")
        .as_u64();
    let settlement_contract = GPv2Settlement::deployed(&web3)
        .await
        .expect("Couldn't load deployed settlement");
    let vault_relayer = settlement_contract
        .vault_relayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");
    let native_token = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    let network = web3
        .net()
        .version()
        .await
        .expect("Failed to retrieve network version ID");
    let network_name = network_name(&network, chain_id);

    let native_token_price_estimation_amount = args
        .amount_to_estimate_prices_with
        .or_else(|| default_amount_to_estimate_prices_with(&network))
        .expect("No amount to estimate prices with set.");

    let vault = match BalancerV2Vault::deployed(&web3).await {
        Ok(contract) => Some(contract),
        Err(DeployError::NotFound(_)) => {
            tracing::warn!("balancer contracts are not deployed on this network");
            None
        }
        Err(err) => panic!("failed to get balancer vault contract: {}", err),
    };

    verify_deployed_contract_constants(&settlement_contract, chain_id)
        .await
        .expect("Deployed contract constants don't match the ones in this binary");
    let domain_separator = DomainSeparator::new(chain_id, settlement_contract.address());
    let postgres = Postgres::new(args.db_url.as_str()).expect("failed to create database");
    let database = Arc::new(database::instrumented::Instrumented::new(
        postgres.clone(),
        metrics.clone(),
    ));

    let sync_start = if args.skip_event_sync {
        web3.eth()
            .block_number()
            .await
            .map(|block| block.as_u64())
            .ok()
    } else {
        None
    };

    let event_updater = Arc::new(EventUpdater::new(
        settlement_contract.clone(),
        database.as_ref().clone(),
        sync_start,
    ));
    let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
        web3.clone(),
        vault.clone(),
        vault_relayer,
        settlement_contract.address(),
    ));

    let gas_price_estimator = Arc::new(InstrumentedGasEstimator::new(
        shared::gas_price_estimation::create_priority_estimator(
            client.clone(),
            &web3,
            args.shared.gas_estimators.as_slice(),
            args.shared.blocknative_api_key.clone(),
        )
        .await
        .expect("failed to create gas price estimator"),
        metrics.clone(),
    ));

    let baseline_sources = args.shared.baseline_sources.unwrap_or_else(|| {
        sources::defaults_for_chain(chain_id).expect("failed to get default baseline sources")
    });
    tracing::info!(?baseline_sources, "using baseline sources");
    let (pair_providers, pool_fetchers): (Vec<_>, Vec<_>) =
        sources::uniswap_like_liquidity_sources(&web3, &baseline_sources)
            .await
            .expect("failed to load baseline source pair providers")
            .values()
            .cloned()
            .unzip();

    let base_tokens = Arc::new(BaseTokens::new(
        native_token.address(),
        &args.shared.base_tokens,
    ));
    let mut allowed_tokens = args.allowed_tokens.clone();
    allowed_tokens.extend(base_tokens.tokens().iter().copied());
    allowed_tokens.push(BUY_ETH_ADDRESS);
    let unsupported_tokens = args.unsupported_tokens.clone();

    let mut finders: Vec<Arc<dyn TokenOwnerFinding>> = pair_providers
        .into_iter()
        .map(|provider| -> Arc<dyn TokenOwnerFinding> {
            Arc::new(UniswapLikePairProviderFinder {
                inner: provider,
                base_tokens: base_tokens.tokens().iter().copied().collect(),
            })
        })
        .collect();
    if let Some(contract) = &vault {
        finders.push(Arc::new(BalancerVaultFinder(contract.clone())));
    }
    let uniswapv3_factory = match IUniswapV3Factory::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };
    if let Some(contract) = uniswapv3_factory {
        finders.push(Arc::new(
            UniswapV3Finder::new(
                contract,
                base_tokens.tokens().iter().copied().collect(),
                current_block,
                args.token_detector_fee_values,
            )
            .await
            .expect("create uniswapv3 finder"),
        ));
    }
    if args.enable_blockscout {
        if let Ok(finder) = BlockscoutTokenOwnerFinder::try_with_network(client.clone(), chain_id) {
            finders.push(Arc::new(finder));
        }
    }
    let trace_call_detector = TraceCallDetector {
        web3: web3.clone(),
        finders,
        settlement_contract: settlement_contract.address(),
    };
    let caching_detector = CachingDetector::new(
        Box::new(trace_call_detector),
        args.token_quality_cache_expiry,
    );
    let bad_token_detector = Arc::new(
        ListBasedDetector::new(
            allowed_tokens,
            unsupported_tokens,
            if args.skip_trace_api {
                UnknownTokenStrategy::Allow
            } else {
                UnknownTokenStrategy::Forward(Box::new(caching_detector))
            },
        )
        .instrumented(),
    );

    let current_block_stream =
        current_block_stream(web3.clone(), args.shared.block_stream_poll_interval_seconds)
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
            metrics.clone(),
        )
        .expect("failed to create pool cache"),
    );
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let balancer_pool_fetcher = if baseline_sources.contains(&BaselineSource::BalancerV2) {
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
        Some(balancer_pool_fetcher)
    } else {
        None
    };
    let zeroex_api = Arc::new(
        DefaultZeroExApi::new(
            args.shared
                .zeroex_url
                .as_deref()
                .unwrap_or(DefaultZeroExApi::DEFAULT_URL),
            args.shared.zeroex_api_key.clone(),
            client.clone(),
        )
        .unwrap(),
    );
    let one_inch_api =
        OneInchClientImpl::new(args.shared.one_inch_url.clone(), client.clone(), chain_id)
            .map(Arc::new);
    let instrumented = |inner: Box<dyn PriceEstimating>, name: String| {
        InstrumentedPriceEstimator::new(inner, name, metrics.clone())
    };
    let create_base_estimator =
        |estimator: PriceEstimatorType| -> (String, Arc<dyn PriceEstimating>) {
            let rate_limiter = Arc::new(RateLimiter::from_strategy(
                args.price_estimation_rate_limiter
                    .clone()
                    .unwrap_or_default(),
                format!("{}_estimator", estimator.name()),
            ));
            let create_http_estimator = |name, base, rate_limiter| -> Box<dyn PriceEstimating> {
                Box::new(HttpPriceEstimator::new(
                    Arc::new(DefaultHttpSolverApi {
                        name,
                        network_name: network_name.to_string(),
                        chain_id,
                        base,
                        client: client.clone(),
                        config: SolverConfig {
                            use_internal_buffers: Some(args.shared.quasimodo_uses_internal_buffers),
                            objective: Some(Objective::SurplusFeesCosts),
                            ..Default::default()
                        },
                    }),
                    pool_fetcher.clone(),
                    balancer_pool_fetcher.clone(),
                    token_info_fetcher.clone(),
                    gas_price_estimator.clone(),
                    native_token.address(),
                    base_tokens.clone(),
                    network_name.to_string(),
                    rate_limiter,
                ))
            };
            let instance: Box<dyn PriceEstimating> = match estimator {
                PriceEstimatorType::Baseline => Box::new(BaselinePriceEstimator::new(
                    pool_fetcher.clone(),
                    gas_price_estimator.clone(),
                    base_tokens.clone(),
                    native_token.address(),
                    native_token_price_estimation_amount,
                )),
                PriceEstimatorType::Paraswap => Box::new(ParaswapPriceEstimator::new(
                    Arc::new(DefaultParaswapApi {
                        client: client.clone(),
                        partner: args.shared.paraswap_partner.clone().unwrap_or_default(),
                        rate_limiter: args.shared.paraswap_rate_limiter.clone().map(|strategy| {
                            RateLimiter::from_strategy(strategy, "paraswap_api".into())
                        }),
                    }),
                    token_info_fetcher.clone(),
                    args.shared.disabled_paraswap_dexs.clone(),
                    rate_limiter,
                )),
                PriceEstimatorType::ZeroEx => Box::new(ZeroExPriceEstimator::new(
                    zeroex_api.clone(),
                    args.shared.disabled_zeroex_sources.clone(),
                    rate_limiter,
                )),
                PriceEstimatorType::Quasimodo => create_http_estimator(
                    "quasimodo-price-estimator".to_string(),
                    args.quasimodo_solver_url.clone().expect(
                        "quasimodo solver url is required when using quasimodo price estimation",
                    ),
                    rate_limiter,
                ),
                PriceEstimatorType::OneInch => Box::new(OneInchPriceEstimator::new(
                    one_inch_api.as_ref().unwrap().clone(),
                    args.shared.disabled_one_inch_protocols.clone(),
                    rate_limiter,
                )),
                PriceEstimatorType::Yearn => create_http_estimator(
                    "yearn-price-estimator".to_string(),
                    args.yearn_solver_url
                        .clone()
                        .expect("yearn solver url is required when using yearn price estimation"),
                    rate_limiter,
                ),
            };

            (
                estimator.name(),
                Arc::new(instrumented(instance, estimator.name())),
            )
        };

    let mut base_estimators_instances: HashMap<_, _> = Default::default();
    let mut get_or_create_base_estimator = move |estimator| {
        base_estimators_instances
            .entry(estimator)
            .or_insert_with(|| create_base_estimator(estimator))
            .clone()
    };

    let sanitized = |estimator| {
        SanitizedPriceEstimator::new(
            estimator,
            native_token.address(),
            bad_token_detector.clone(),
        )
    };

    let price_estimator = Arc::new(sanitized(Box::new(CompetitionPriceEstimator::new(
        args.price_estimators
            .iter()
            .map(|estimator| get_or_create_base_estimator(*estimator))
            .collect(),
    ))));

    let fast_price_estimator = Arc::new(sanitized(Box::new(RacingCompetitionPriceEstimator::new(
        args.price_estimators
            .iter()
            .map(|estimator| get_or_create_base_estimator(*estimator))
            .collect(),
        args.fast_price_estimation_results_required,
    ))));

    let native_price_estimator = Arc::new(CachingNativePriceEstimator::new(
        Box::new(NativePriceEstimator::new(
            Arc::new(sanitized(Box::new(CompetitionPriceEstimator::new(
                args.native_price_estimators
                    .iter()
                    .map(|estimator| create_base_estimator(*estimator))
                    .collect(),
            )))),
            native_token.address(),
            native_token_price_estimation_amount,
        )),
        args.native_price_cache_max_age_secs,
        metrics.clone(),
    ));
    native_price_estimator.spawn_maintenance_task(
        Duration::from_secs(1),
        Some(args.native_price_cache_max_update_size),
    );

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
        CowSubsidy::new(token, vtoken, args.cow_fee_factors.unwrap_or_default())
    });

    let fee_subsidy_config = Arc::new(FeeSubsidyConfiguration {
        fee_discount: args.fee_discount,
        min_discounted_fee: args.min_discounted_fee,
        fee_factor: args.fee_factor,
        liquidity_order_owners: args.liquidity_order_owners.iter().copied().collect(),
        partner_additional_fee_factors: args.partner_additional_fee_factors.clone(),
    }) as Arc<dyn FeeSubsidizing>;

    let fee_subsidy = match cow_subsidy {
        Some(cow_subsidy) => Arc::new(FeeSubsidies(vec![
            fee_subsidy_config,
            Arc::new(cow_subsidy),
        ])),
        None => fee_subsidy_config,
    };

    let create_quoter = |price_estimator: Arc<dyn PriceEstimating>,
                         storage: Arc<dyn QuoteStoring>| {
        Arc::new(OrderQuoter::new(
            price_estimator,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
            fee_subsidy.clone(),
            storage,
        ))
    };
    let optimal_quoter = create_quoter(price_estimator.clone(), database.clone());
    let fast_quoter = create_quoter(fast_price_estimator.clone(), Arc::new(Forget));

    let solvable_orders_cache = SolvableOrdersCache::new(
        args.min_order_validity_period,
        database.clone(),
        args.banned_users.iter().copied().collect(),
        balance_fetcher.clone(),
        bad_token_detector.clone(),
        current_block_stream.clone(),
        native_price_estimator.clone(),
        metrics.clone(),
    );
    let block = current_block_stream.borrow().number.unwrap().as_u64();
    solvable_orders_cache
        .update(block)
        .await
        .expect("failed to perform initial solvable orders update");
    let order_validator = Arc::new(OrderValidator::new(
        Box::new(web3.clone()),
        native_token.clone(),
        args.banned_users.iter().copied().collect(),
        args.liquidity_order_owners.iter().copied().collect(),
        args.min_order_validity_period,
        args.max_order_validity_period,
        args.enable_presign_orders,
        bad_token_detector.clone(),
        Arc::new(MinFeeCalculator::new(
            price_estimator.clone(),
            gas_price_estimator.clone(),
            database.clone(),
            bad_token_detector.clone(),
            fee_subsidy.clone(),
            native_price_estimator,
            args.liquidity_order_owners.iter().copied().collect(),
        )),
        balance_fetcher,
    ));
    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        settlement_contract.address(),
        database.clone(),
        bad_token_detector,
        solvable_orders_cache.clone(),
        args.solvable_orders_max_update_age,
        order_validator.clone(),
    ));
    let mut service_maintainer = ServiceMaintenance {
        maintainers: vec![
            database.clone(),
            event_updater,
            pool_fetcher,
            solvable_orders_cache,
        ],
    };
    if let Some(balancer) = balancer_pool_fetcher {
        service_maintainer.maintainers.push(balancer);
    }
    check_database_connection(orderbook.as_ref()).await;
    let quotes =
        Arc::new(QuoteHandler::new(order_validator, optimal_quoter).with_fast_quoter(fast_quoter));
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let solver_competition = Arc::new(SolverCompetition::default());
    let serve_api = serve_api(
        database.clone(),
        orderbook.clone(),
        quotes,
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
        solver_competition,
        args.shared.solver_competition_auth,
    );
    let maintenance_task =
        task::spawn(service_maintainer.run_maintenance_on_new_block(current_block_stream));
    let db_metrics_task = task::spawn(database_metrics(metrics, postgres));

    let mut metrics_address = args.bind_address;
    metrics_address.set_port(DEFAULT_METRICS_PORT);
    tracing::info!(%metrics_address, "serving metrics");
    let metrics_task = serve_metrics(orderbook, metrics_address);

    futures::pin_mut!(serve_api);
    tokio::select! {
        result = &mut serve_api => tracing::error!(?result, "API task exited"),
        result = maintenance_task => tracing::error!(?result, "maintenance task exited"),
        result = db_metrics_task => tracing::error!(?result, "database metrics task exited"),
        result = metrics_task => tracing::error!(?result, "metrics task exited"),
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

async fn check_database_connection(orderbook: &Orderbook) {
    orderbook
        .get_orders(&OrderFilter {
            uid: Some(OrderUid::default()),
            ..Default::default()
        })
        .await
        .expect("failed to connect to database");
}

fn default_amount_to_estimate_prices_with(network_id: &str) -> Option<U256> {
    match network_id {
        // Mainnet, Rinkeby
        "1" | "4" => Some(10u128.pow(18).into()),
        // Xdai
        "100" => Some(10u128.pow(21).into()),
        _ => None,
    }
}
