pub mod arguments;
pub mod database;
pub mod event_updater;
pub mod solvable_orders;

use crate::{
    database::{
        onchain_order_events::{ethflow_events::EthFlowOnchainOrderParser, OnchainOrderParser},
        Postgres,
    },
    event_updater::{CoWSwapOnchainOrdersContract, EventUpdater, GPv2SettlementContract},
    solvable_orders::SolvableOrdersCache,
};
use contracts::{
    BalancerV2Vault, CowProtocolToken, CowProtocolVirtualToken, IUniswapV3Factory, WETH9,
};
use ethcontract::{errors::DeployError, BlockId, BlockNumber};
use model::DomainSeparator;
use shared::{
    account_balances::Web3BalanceFetcher,
    bad_token::{
        cache::CachingDetector,
        instrumented::InstrumentedBadTokenDetectorExt,
        list_based::{ListBasedDetector, UnknownTokenStrategy},
        token_owner_finder,
        trace_call::TraceCallDetector,
    },
    balancer_sor_api::DefaultBalancerSorApi,
    baseline_solver::BaseTokens,
    fee_subsidy::{
        config::FeeSubsidyConfiguration, cow_token::CowSubsidy, FeeSubsidies, FeeSubsidizing,
    },
    gas_price::InstrumentedGasEstimator,
    http_client::HttpClientFactory,
    http_solver::{DefaultHttpSolverApi, SolverConfig},
    maintenance::Maintaining,
    metrics::LivenessChecking,
    oneinch_api::OneInchClientImpl,
    order_quoting::OrderQuoter,
    paraswap_api::DefaultParaswapApi,
    price_estimation::{
        balancer_sor::BalancerSor, baseline::BaselinePriceEstimator,
        competition::CompetitionPriceEstimator, http::HttpPriceEstimator,
        instrumented::InstrumentedPriceEstimator, native::NativePriceEstimator,
        native_price_cache::CachingNativePriceEstimator, oneinch::OneInchPriceEstimator,
        paraswap::ParaswapPriceEstimator, sanitized::SanitizedPriceEstimator,
        zeroex::ZeroExPriceEstimator, PriceEstimating, PriceEstimatorType,
    },
    rate_limiter::RateLimiter,
    recent_block_cache::CacheConfig,
    signature_validator::Web3SignatureValidator,
    sources::{
        balancer_v2::{pool_fetching::BalancerContracts, BalancerFactoryKind, BalancerPoolFetcher},
        uniswap_v2::pool_cache::PoolCache,
        uniswap_v3::pool_fetching::UniswapV3PoolFetcher,
        BaselineSource, PoolAggregator,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    zeroex_api::DefaultZeroExApi,
};
use std::{collections::HashMap, sync::Arc, time::Duration};

struct Liveness {
    solvable_orders_cache: Arc<SolvableOrdersCache>,
    max_auction_age: Duration,
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        let age = self.solvable_orders_cache.last_update_time().elapsed();
        age <= self.max_auction_age
    }
}

/// Assumes tracing and metrics registry have already been set up.
pub async fn main(args: arguments::Arguments) {
    let db = Postgres::new(args.db_url.as_str()).await.unwrap();
    let db_metrics = crate::database::database_metrics(db.clone());

    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::web3(&http_factory, &args.shared.node_url, "base");

    let current_block_stream = shared::current_block::current_block_stream(
        web3.clone(),
        args.shared.block_stream_poll_interval_seconds,
    )
    .await
    .unwrap();

    let settlement_contract = contracts::GPv2Settlement::deployed(&web3)
        .await
        .expect("Couldn't load deployed settlement");
    // The events from the ethflow contract are read with the more generic contract
    // interface called CoWSwapOnchainOrders.
    let cowswap_onchain_order_contract_for_eth_flow =
        contracts::CoWSwapOnchainOrders::at(&web3, args.ethflow_contract);
    let vault_relayer = settlement_contract
        .vault_relayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");
    let native_token = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");
    let vault = match BalancerV2Vault::deployed(&web3).await {
        Ok(contract) => Some(contract),
        Err(DeployError::NotFound(_)) => {
            tracing::warn!("balancer contracts are not deployed on this network");
            None
        }
        Err(err) => panic!("failed to get balancer vault contract: {}", err),
    };
    let uniswapv3_factory = match IUniswapV3Factory::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };

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
    let network_name = shared::network::network_name(&network, chain_id);

    let signature_validator = Arc::new(Web3SignatureValidator::new(web3.clone()));

    let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
        web3.clone(),
        vault.clone(),
        vault_relayer,
        settlement_contract.address(),
    ));

    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            &http_factory,
            &web3,
            args.shared.gas_estimators.as_slice(),
            args.shared.blocknative_api_key.clone(),
        )
        .await
        .expect("failed to create gas price estimator"),
    );

    let baseline_sources = args.shared.baseline_sources.unwrap_or_else(|| {
        shared::sources::defaults_for_chain(chain_id)
            .expect("failed to get default baseline sources")
    });
    tracing::info!(?baseline_sources, "using baseline sources");
    let (pair_providers, pool_fetchers): (Vec<_>, Vec<_>) =
        shared::sources::uniswap_like_liquidity_sources(&web3, &baseline_sources)
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
    allowed_tokens.push(model::order::BUY_ETH_ADDRESS);
    let unsupported_tokens = args.unsupported_tokens.clone();

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
                web3: shared::web3(&http_factory, tracing_node_url, "trace"),
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
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let balancer_pool_fetcher = if baseline_sources.contains(&BaselineSource::BalancerV2) {
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
        Some(balancer_pool_fetcher)
    } else {
        None
    };
    let uniswap_v3_pool_fetcher = if baseline_sources.contains(&BaselineSource::UniswapV3) {
        let uniswap_v3_pool_fetcher = Arc::new(
            UniswapV3PoolFetcher::new(
                chain_id,
                args.shared.liquidity_fetcher_max_age_update,
                http_factory.create(),
            )
            .await
            .expect("failed to create UniswapV3 pool fetcher in orderbook"),
        );
        Some(uniswap_v3_pool_fetcher)
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
    let instrumented = |inner: Box<dyn PriceEstimating>, name: String| {
        InstrumentedPriceEstimator::new(inner, name)
    };
    let balancer_sor_api = args.balancer_sor_url.map(|url| {
        Arc::new(DefaultBalancerSorApi::new(http_factory.create(), url, chain_id).unwrap())
    });
    let native_token_price_estimation_amount = args
        .amount_to_estimate_prices_with
        .or_else(|| {
            shared::price_estimation::native::default_amount_to_estimate_native_prices_with(
                &network,
            )
        })
        .expect("No amount to estimate prices with set.");

    let create_base_estimator =
        |estimator: PriceEstimatorType| -> (String, Arc<dyn PriceEstimating>) {
            let rate_limiter = |name| {
                Arc::new(RateLimiter::from_strategy(
                    args.price_estimation_rate_limiter
                        .clone()
                        .unwrap_or_default(),
                    format!("{}_estimator", &name),
                ))
            };
            let create_http_estimator = |name, base| -> Box<dyn PriceEstimating> {
                Box::new(HttpPriceEstimator::new(
                    Arc::new(DefaultHttpSolverApi {
                        name,
                        network_name: network_name.to_string(),
                        chain_id,
                        base,
                        client: http_factory.create(),
                        config: SolverConfig {
                            use_internal_buffers: Some(args.shared.quasimodo_uses_internal_buffers),
                            objective: Some(shared::http_solver::Objective::SurplusFeesCosts),
                            ..Default::default()
                        },
                    }),
                    pool_fetcher.clone(),
                    balancer_pool_fetcher.clone(),
                    uniswap_v3_pool_fetcher.clone(),
                    token_info_fetcher.clone(),
                    gas_price_estimator.clone(),
                    native_token.address(),
                    base_tokens.clone(),
                    network_name.to_string(),
                    rate_limiter(estimator.name()),
                ))
            };
            let instance: Box<dyn PriceEstimating> = match estimator {
                PriceEstimatorType::Baseline => Box::new(BaselinePriceEstimator::new(
                    pool_fetcher.clone(),
                    gas_price_estimator.clone(),
                    base_tokens.clone(),
                    native_token.address(),
                    native_token_price_estimation_amount,
                    rate_limiter(estimator.name()),
                )),
                PriceEstimatorType::Paraswap => Box::new(ParaswapPriceEstimator::new(
                    Arc::new(DefaultParaswapApi {
                        client: http_factory.create(),
                        partner: args.shared.paraswap_partner.clone().unwrap_or_default(),
                        rate_limiter: args.shared.paraswap_rate_limiter.clone().map(|strategy| {
                            RateLimiter::from_strategy(strategy, "paraswap_api".into())
                        }),
                    }),
                    token_info_fetcher.clone(),
                    args.shared.disabled_paraswap_dexs.clone(),
                    rate_limiter(estimator.name()),
                )),
                PriceEstimatorType::ZeroEx => Box::new(ZeroExPriceEstimator::new(
                    zeroex_api.clone(),
                    args.shared.disabled_zeroex_sources.clone(),
                    rate_limiter(estimator.name()),
                )),
                PriceEstimatorType::Quasimodo => create_http_estimator(
                    "quasimodo-price-estimator".to_string(),
                    args.quasimodo_solver_url.clone().expect(
                        "quasimodo solver url is required when using quasimodo price estimation",
                    ),
                ),
                PriceEstimatorType::OneInch => Box::new(OneInchPriceEstimator::new(
                    one_inch_api.as_ref().unwrap().clone(),
                    args.shared.disabled_one_inch_protocols.clone(),
                    rate_limiter(estimator.name()),
                    args.shared.one_inch_referrer_address
                )),
                PriceEstimatorType::Yearn => create_http_estimator(
                    "yearn-price-estimator".to_string(),
                    args.yearn_solver_url
                        .clone()
                        .expect("yearn solver url is required when using yearn price estimation"),
                ),
                PriceEstimatorType::BalancerSor => Box::new(BalancerSor::new(
                    balancer_sor_api.clone().expect("trying to create BalancerSor price estimator but didn't get balancer sor url"),
                    rate_limiter(estimator.name()),
                    gas_price_estimator.clone(),
                )),
            };

            (
                estimator.name(),
                Arc::new(instrumented(instance, estimator.name())),
            )
        };
    let sanitized = |estimator| {
        SanitizedPriceEstimator::new(
            estimator,
            native_token.address(),
            bad_token_detector.clone(),
        )
    };
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
    ));

    let solvable_orders_cache = SolvableOrdersCache::new(
        args.min_order_validity_period,
        db.clone(),
        args.banned_users.iter().copied().collect(),
        balance_fetcher.clone(),
        bad_token_detector.clone(),
        current_block_stream.clone(),
        native_price_estimator.clone(),
        signature_validator.clone(),
        Duration::from_secs(2),
    );
    let block = current_block_stream.borrow().number.unwrap().as_u64();
    solvable_orders_cache
        .update(block)
        .await
        .expect("failed to perform initial solvable orders update");

    let sync_start = if args.skip_event_sync {
        web3.eth()
            .block(BlockId::Number(BlockNumber::Latest))
            .await
            .ok()
            .flatten()
            .map(|block| {
                (
                    block.number.expect("number must exist").as_u64(),
                    block.hash.expect("hash must exist"),
                )
            })
    } else {
        None
    };
    let event_updater = Arc::new(EventUpdater::new(
        GPv2SettlementContract::new(settlement_contract.clone()),
        db.clone(),
        settlement_contract.clone().raw_instance().web3(),
        sync_start,
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
        partner_additional_fee_factors: args.order_quoting.partner_additional_fee_factors.clone(),
    }) as Arc<dyn FeeSubsidizing>;

    let fee_subsidy = match cow_subsidy {
        Some(cow_subsidy) => Arc::new(FeeSubsidies(vec![
            fee_subsidy_config,
            Arc::new(cow_subsidy),
        ])),
        None => fee_subsidy_config,
    };
    let mut base_estimators_instances: HashMap<_, _> = Default::default();
    let mut get_or_create_base_estimator = move |estimator| {
        base_estimators_instances
            .entry(estimator)
            .or_insert_with(|| create_base_estimator(estimator))
            .clone()
    };
    let price_estimator = Arc::new(sanitized(Box::new(CompetitionPriceEstimator::new(
        args.order_quoting
            .price_estimators
            .iter()
            .map(|estimator| get_or_create_base_estimator(*estimator))
            .collect(),
    ))));
    let database = Arc::new(db.clone());
    let quoter = OrderQuoter::new(
        price_estimator.clone(),
        native_price_estimator.clone(),
        gas_price_estimator,
        fee_subsidy,
        database.clone(),
        chrono::Duration::from_std(args.order_quoting.eip1271_onchain_quote_validity_seconds)
            .unwrap(),
        chrono::Duration::from_std(args.order_quoting.presign_onchain_quote_validity_seconds)
            .unwrap(),
    );
    let custom_ethflow_order_parser = EthFlowOnchainOrderParser {};
    let onchain_order_event_parser = OnchainOrderParser::new(
        db.clone(),
        Box::new(quoter),
        Box::new(custom_ethflow_order_parser),
        DomainSeparator::new(chain_id, settlement_contract.address()),
        settlement_contract.address(),
    );
    let broadcaster_event_updater = Arc::new(EventUpdater::new(
        CoWSwapOnchainOrdersContract::new(cowswap_onchain_order_contract_for_eth_flow.clone()),
        onchain_order_event_parser,
        cowswap_onchain_order_contract_for_eth_flow
            .clone()
            .raw_instance()
            .web3(),
        sync_start,
    ));
    let maintainers: Vec<Arc<dyn Maintaining>> = if args.enable_ethflow_orders {
        vec![
            pool_fetcher,
            event_updater,
            broadcaster_event_updater,
            Arc::new(db.clone()),
        ]
    } else {
        vec![pool_fetcher, event_updater, Arc::new(db.clone())]
    };
    let mut service_maintainer = shared::maintenance::ServiceMaintenance { maintainers };

    if let Some(balancer) = balancer_pool_fetcher {
        service_maintainer.maintainers.push(balancer);
    }
    if let Some(uniswap_v3) = uniswap_v3_pool_fetcher {
        service_maintainer.maintainers.push(uniswap_v3);
    }
    let maintenance_task =
        tokio::task::spawn(service_maintainer.run_maintenance_on_new_block(current_block_stream));

    let liveness = Liveness {
        max_auction_age: args.max_auction_age,
        solvable_orders_cache,
    };
    let serve_metrics = shared::metrics::serve_metrics(Arc::new(liveness), args.metrics_address);

    tokio::select! {
        result = serve_metrics => tracing::error!(?result, "serve_metrics exited"),
        _ = db_metrics => unreachable!(),
        _ = maintenance_task => unreachable!(),
    };
}
