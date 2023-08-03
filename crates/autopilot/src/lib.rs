use {
    crate::{
        database::{
            ethflow_events::event_retriever::EthFlowRefundRetriever,
            onchain_order_events::{
                ethflow_events::{determine_ethflow_indexing_start, EthFlowOnchainOrderParser},
                event_retriever::CoWSwapOnchainOrdersContract,
                OnchainOrderParser,
            },
            Postgres,
        },
        event_updater::{EventUpdater, GPv2SettlementContract},
        fok_limit_orders::{LimitOrderMetrics, LimitOrderQuoter},
        solvable_orders::SolvableOrdersCache,
    },
    contracts::{
        BalancerV2Vault,
        CowProtocolToken,
        CowProtocolVirtualToken,
        IUniswapV3Factory,
        WETH9,
    },
    ethcontract::{errors::DeployError, BlockNumber},
    futures::StreamExt,
    model::DomainSeparator,
    shared::{
        account_balances,
        bad_token::{
            cache::CachingDetector,
            instrumented::InstrumentedBadTokenDetectorExt,
            list_based::{ListBasedDetector, UnknownTokenStrategy},
            token_owner_finder,
            trace_call::TraceCallDetector,
        },
        baseline_solver::BaseTokens,
        current_block::block_number_to_block_number_hash,
        fee_subsidy::{
            config::FeeSubsidyConfiguration,
            cow_token::CowSubsidy,
            FeeSubsidies,
            FeeSubsidizing,
        },
        gas_price::InstrumentedGasEstimator,
        http_client::HttpClientFactory,
        maintenance::{Maintaining, ServiceMaintenance},
        metrics::LivenessChecking,
        oneinch_api::OneInchClientImpl,
        order_quoting::OrderQuoter,
        price_estimation::factory::{self, PriceEstimatorFactory, PriceEstimatorSource},
        recent_block_cache::CacheConfig,
        signature_validator::Web3SignatureValidator,
        sources::{
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
        token_list::{AutoUpdatingTokenList, TokenListConfiguration},
        zeroex_api::DefaultZeroExApi,
    },
    std::{collections::HashSet, sync::Arc, time::Duration},
    tracing::Instrument,
};

pub mod arguments;
pub mod database;
pub mod decoded_settlement;
pub mod driver_api;
pub mod driver_model;
pub mod event_updater;
pub mod fok_limit_orders;
pub mod on_settlement_event_updater;
pub mod run_loop;
pub mod solvable_orders;

/// To never get to the state where a limit order can not be considered usable
/// because the `surplus_fee` is too old the `surplus_fee` is valid for longer
/// than its update interval. This factor controls how much longer it's
/// considered valid. If the `surplus_fee` gets updated every 5 minutes and the
/// factor is 2 we consider limit orders valid where the `surplus_fee` was
/// computed up to 10 minutes ago.
const SURPLUS_FEE_EXPIRATION_FACTOR: u8 = 2;

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
    tokio::task::spawn(
        crate::database::database_metrics(db.clone())
            .instrument(tracing::info_span!("database_metrics")),
    );

    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::ethrpc::web3(
        &args.shared.ethrpc,
        &http_factory,
        &args.shared.node_url,
        "base",
    );
    let simulation_web3 = args.shared.simulation_node_url.as_ref().map(|node_url| {
        shared::ethrpc::web3(&args.shared.ethrpc, &http_factory, node_url, "simulation")
    });

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

    let current_block_stream = args
        .shared
        .current_block
        .stream(web3.clone())
        .await
        .unwrap();

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
    let uniswapv3_factory = match IUniswapV3Factory::deployed(&web3).await {
        Err(DeployError::NotFound(_)) => None,
        other => Some(other.unwrap()),
    };

    let network = web3
        .net()
        .version()
        .await
        .expect("Failed to retrieve network version ID");
    let network_name = shared::network::network_name(&network, chain_id);
    let network_time_between_blocks = args
        .shared
        .network_block_interval
        .or_else(|| shared::network::block_interval(&network, chain_id))
        .expect("unknown network block interval");

    let signature_validator = Arc::new(Web3SignatureValidator::new(web3.clone()));

    let balance_fetcher = args.shared.balances.cached(
        account_balances::Contracts {
            chain_id,
            settlement: settlement_contract.address(),
            vault_relayer,
            vault: vault.as_ref().map(|contract| contract.address()),
        },
        web3.clone(),
        simulation_web3.clone(),
        args.shared
            .tenderly
            .get_api_instance(&http_factory, "balance_fetching".into())
            .unwrap(),
        current_block_stream.clone(),
    );

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

    let baseline_sources = args.shared.baseline_sources.clone().unwrap_or_else(|| {
        shared::sources::defaults_for_chain(chain_id)
            .expect("failed to get default baseline sources")
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
    let block_retriever = args.shared.current_block.retriever(web3.clone());
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
                block_retriever.clone(),
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
                block_retriever,
                args.shared.max_pools_to_initialize_cache,
            )
            .await
            .expect("error initializing Uniswap V3 pool fetcher"),
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
        .price_estimator(&PriceEstimatorSource::for_args(
            args.order_quoting.price_estimators.as_slice(),
            &args.order_quoting.price_estimation_drivers,
            &args.order_quoting.price_estimation_legacy_solvers,
        ))
        .unwrap();
    let native_price_estimator = price_estimator_factory
        .native_price_estimator(&PriceEstimatorSource::for_args(
            args.native_price_estimators.as_slice(),
            &args.order_quoting.price_estimation_drivers,
            &args.order_quoting.price_estimation_legacy_solvers,
        ))
        .unwrap();

    let skip_event_sync_start = if args.skip_event_sync {
        block_number_to_block_number_hash(&web3, BlockNumber::Latest).await
    } else {
        None
    };
    let block_retriever = args.shared.current_block.retriever(web3.clone());
    let event_updater = Arc::new(EventUpdater::new(
        GPv2SettlementContract::new(settlement_contract.clone()),
        db.clone(),
        block_retriever.clone(),
        skip_event_sync_start,
    ));
    let mut maintainers: Vec<Arc<dyn Maintaining>> =
        vec![pool_fetcher.clone(), event_updater, Arc::new(db.clone())];

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
    let liquidity_order_owners: HashSet<_> = args
        .order_quoting
        .liquidity_order_owners
        .iter()
        .copied()
        .collect();
    let fee_subsidy_config = Arc::new(FeeSubsidyConfiguration {
        fee_discount: args.order_quoting.fee_discount,
        min_discounted_fee: args.order_quoting.min_discounted_fee,
        fee_factor: args.order_quoting.fee_factor,
        liquidity_order_owners: liquidity_order_owners.clone(),
    }) as Arc<dyn FeeSubsidizing>;

    let fee_subsidy = match cow_subsidy {
        Some(cow_subsidy) => Arc::new(FeeSubsidies(vec![
            fee_subsidy_config,
            Arc::new(cow_subsidy),
        ])),
        None => fee_subsidy_config,
    };
    let quoter = Arc::new(OrderQuoter::new(
        price_estimator,
        native_price_estimator.clone(),
        gas_price_estimator,
        fee_subsidy,
        Arc::new(db.clone()),
        chrono::Duration::from_std(args.order_quoting.eip1271_onchain_quote_validity_seconds)
            .unwrap(),
        chrono::Duration::from_std(args.order_quoting.presign_onchain_quote_validity_seconds)
            .unwrap(),
    ));

    if let Some(ethflow_contract) = args.ethflow_contract {
        let start_block = determine_ethflow_indexing_start(
            &skip_event_sync_start,
            args.ethflow_indexing_start,
            &web3,
            chain_id,
        )
        .await;

        let refund_event_handler = Arc::new(
            EventUpdater::new_skip_blocks_before(
                // This cares only about ethflow refund events because all the other ethflow
                // events are already indexed by the OnchainOrderParser.
                EthFlowRefundRetriever::new(web3.clone(), ethflow_contract),
                db.clone(),
                block_retriever.clone(),
                start_block,
            )
            .await
            .unwrap(),
        );
        maintainers.push(refund_event_handler);

        let custom_ethflow_order_parser = EthFlowOnchainOrderParser {};
        let onchain_order_event_parser = OnchainOrderParser::new(
            db.clone(),
            web3.clone(),
            quoter.clone(),
            Box::new(custom_ethflow_order_parser),
            DomainSeparator::new(chain_id, settlement_contract.address()),
            settlement_contract.address(),
            liquidity_order_owners,
        );
        let broadcaster_event_updater = Arc::new(
            EventUpdater::new_skip_blocks_before(
                // The events from the ethflow contract are read with the more generic contract
                // interface called CoWSwapOnchainOrders.
                CoWSwapOnchainOrdersContract::new(web3.clone(), ethflow_contract),
                onchain_order_event_parser,
                block_retriever,
                start_block,
            )
            .await
            .expect("Should be able to initialize event updater. Database read issues?"),
        );
        maintainers.push(broadcaster_event_updater);
    }
    if let Some(balancer) = balancer_pool_fetcher {
        maintainers.push(balancer);
    }
    if let Some(uniswap_v3) = uniswap_v3_pool_fetcher {
        maintainers.push(uniswap_v3);
    }

    let service_maintainer = ServiceMaintenance::new(maintainers);
    tokio::task::spawn(
        service_maintainer.run_maintenance_on_new_block(current_block_stream.clone()),
    );

    let block = current_block_stream.borrow().number;
    let solvable_orders_cache = SolvableOrdersCache::new(
        args.min_order_validity_period,
        db.clone(),
        args.banned_users.iter().copied().collect(),
        balance_fetcher.clone(),
        bad_token_detector.clone(),
        current_block_stream.clone(),
        native_price_estimator.clone(),
        signature_validator.clone(),
        args.auction_update_interval,
        args.ethflow_contract,
        args.max_surplus_fee_age * SURPLUS_FEE_EXPIRATION_FACTOR.into(),
        args.limit_order_price_factor
            .try_into()
            .expect("limit order price factor can't be converted to BigDecimal"),
        !args.enable_colocation,
        args.fee_objective_scaling_factor,
    );
    solvable_orders_cache
        .update(block)
        .await
        .expect("failed to perform initial solvable orders update");
    let liveness = Liveness {
        max_auction_age: args.max_auction_age,
        solvable_orders_cache: solvable_orders_cache.clone(),
    };
    let serve_metrics = shared::metrics::serve_metrics(Arc::new(liveness), args.metrics_address);

    let on_settlement_event_updater =
        crate::on_settlement_event_updater::OnSettlementEventUpdater {
            web3: web3.clone(),
            contract: settlement_contract,
            native_token: native_token.address(),
            db: db.clone(),
            current_block: current_block_stream.clone(),
        };
    tokio::task::spawn(
        on_settlement_event_updater
            .run_forever()
            .instrument(tracing::info_span!("on_settlement_event_updater")),
    );

    if args.process_fill_or_kill_limit_orders {
        let limit_order_age = chrono::Duration::from_std(args.max_surplus_fee_age).unwrap();
        LimitOrderQuoter {
            limit_order_age,
            quoter,
            database: db.clone(),
            parallelism: args.limit_order_quoter_parallelism,
            balance_fetcher: balance_fetcher.clone(),
            strategies: args.quoting_strategies,
            batch_size: args.limit_order_quoter_batch_size,
        }
        .spawn();
        LimitOrderMetrics {
            quoting_age: limit_order_age,
            validity_age: limit_order_age * SURPLUS_FEE_EXPIRATION_FACTOR.into(),
            database: db.clone(),
        }
        .spawn();
    }

    if args.enable_colocation {
        if args.drivers.is_empty() {
            panic!("colocation is enabled but no drivers are configured");
        }
        let market_makable_token_list_configuration = TokenListConfiguration {
            url: args.trusted_tokens_url,
            update_interval: args.trusted_tokens_update_interval,
            chain_id,
            client: http_factory.create(),
            hardcoded: args.trusted_tokens.unwrap_or_default(),
        };
        // updated in background task
        let market_makable_token_list =
            AutoUpdatingTokenList::from_configuration(market_makable_token_list_configuration)
                .await;
        let run = run_loop::RunLoop {
            solvable_orders_cache,
            database: db,
            drivers: args
                .drivers
                .into_iter()
                .map(driver_api::Driver::new)
                .collect(),
            current_block: current_block_stream,
            web3,
            network_block_interval: network_time_between_blocks,
            market_makable_token_list,
            submission_deadline: args.submission_deadline as u64,
            additional_deadline_for_rewards: args.additional_deadline_for_rewards as u64,
            token_info: token_info_fetcher,
        };
        run.run_forever().await;
        unreachable!("run loop exited");
    } else {
        let result = serve_metrics.await;
        unreachable!("serve_metrics exited {result:?}");
    }
}
