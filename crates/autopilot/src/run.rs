use {
    crate::{
        arguments::Arguments,
        boundary,
        database::{
            ethflow_events::event_retriever::EthFlowRefundRetriever,
            onchain_order_events::{
                ethflow_events::{
                    determine_ethflow_indexing_start,
                    determine_ethflow_refund_indexing_start,
                    EthFlowOnchainOrderParser,
                },
                event_retriever::CoWSwapOnchainOrdersContract,
                OnchainOrderParser,
            },
            Postgres,
        },
        domain,
        event_updater::EventUpdater,
        infra,
        maintenance::Maintenance,
        run_loop::{self, RunLoop},
        shadow,
        solvable_orders::SolvableOrdersCache,
    },
    chain::Chain,
    clap::Parser,
    contracts::{BalancerV2Vault, IUniswapV3Factory},
    ethcontract::{common::DeploymentInformation, dyns::DynWeb3, errors::DeployError, BlockNumber},
    ethrpc::block_stream::block_number_to_block_number_hash,
    futures::StreamExt,
    model::DomainSeparator,
    observe::metrics::LivenessChecking,
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
        code_fetching::CachedCodeFetcher,
        http_client::HttpClientFactory,
        maintenance::ServiceMaintenance,
        order_quoting::{self, OrderQuoter},
        price_estimation::factory::{self, PriceEstimatorFactory},
        signature_validator,
        sources::{uniswap_v2::UniV2BaselineSourceParameters, BaselineSource},
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
        token_list::{AutoUpdatingTokenList, TokenListConfiguration},
    },
    std::{
        sync::{Arc, RwLock},
        time::{Duration, Instant},
    },
    tracing::Instrument,
    url::Url,
};

pub struct Liveness {
    max_auction_age: Duration,
    last_auction_time: RwLock<Instant>,
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        let last_auction_time = self.last_auction_time.read().unwrap();
        let auction_age = last_auction_time.elapsed();
        auction_age <= self.max_auction_age
    }
}

impl Liveness {
    pub fn new(max_auction_age: Duration) -> Liveness {
        Liveness {
            max_auction_age,
            last_auction_time: RwLock::new(Instant::now()),
        }
    }

    pub fn auction(&self) {
        *self.last_auction_time.write().unwrap() = Instant::now();
    }
}

async fn ethrpc(url: &Url, ethrpc_args: &shared::ethrpc::Arguments) -> infra::blockchain::Rpc {
    infra::blockchain::Rpc::new(url, ethrpc_args)
        .await
        .expect("connect ethereum RPC")
}

async fn ethereum(
    web3: DynWeb3,
    chain: &Chain,
    url: Url,
    contracts: infra::blockchain::contracts::Addresses,
    poll_interval: Duration,
) -> infra::Ethereum {
    infra::Ethereum::new(web3, chain, url, contracts, poll_interval).await
}

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    observe::tracing::initialize(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
    );
    observe::panic_hook::install();
    tracing::info!("running autopilot with validated arguments:\n{}", args);
    observe::metrics::setup_registry(Some("gp_v2_autopilot".into()), None);

    if args.drivers.is_empty() {
        panic!("colocation is enabled but no drivers are configured");
    }

    if args.shadow.is_some() {
        shadow_mode(args).await;
    } else {
        run(args).await;
    }
}

/// Assumes tracing and metrics registry have already been set up.
pub async fn run(args: Arguments) {
    assert!(args.shadow.is_none(), "cannot run in shadow mode");

    let db = Postgres::new(args.db_url.as_str(), args.insert_batch_size)
        .await
        .unwrap();
    crate::database::run_database_metrics_work(db.clone());

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

    let ethrpc = ethrpc(&args.shared.node_url, &args.shared.ethrpc).await;
    let chain = ethrpc.chain();
    let web3 = ethrpc.web3().clone();
    let url = ethrpc.url().clone();
    let contracts = infra::blockchain::contracts::Addresses {
        settlement: args.shared.settlement_contract_address,
        weth: args.shared.native_token_address,
    };
    let eth = ethereum(
        web3.clone(),
        &chain,
        url,
        contracts.clone(),
        args.shared.current_block.block_stream_poll_interval,
    )
    .await;

    let vault_relayer = eth
        .contracts()
        .settlement()
        .vault_relayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");
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

    let chain = Chain::try_from(chain_id).expect("incorrect chain ID");

    let signature_validator = signature_validator::validator(
        &web3,
        signature_validator::Contracts {
            settlement: eth.contracts().settlement().address(),
            vault_relayer,
        },
    );

    let balance_fetcher = account_balances::cached(
        &web3,
        account_balances::Contracts {
            settlement: eth.contracts().settlement().address(),
            vault_relayer,
            vault: vault.as_ref().map(|contract| contract.address()),
        },
        eth.current_block().clone(),
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

    let baseline_sources = args
        .shared
        .baseline_sources
        .clone()
        .unwrap_or_else(|| shared::sources::defaults_for_network(&chain));
    tracing::info!(?baseline_sources, "using baseline sources");
    let univ2_sources = baseline_sources
        .iter()
        .filter_map(|source: &BaselineSource| {
            UniV2BaselineSourceParameters::from_baseline_source(*source, &chain_id.to_string())
        })
        .chain(args.shared.custom_univ2_baseline_sources.iter().copied());
    let pair_providers: Vec<_> = futures::stream::iter(univ2_sources)
        .then(|source: UniV2BaselineSourceParameters| {
            let web3 = &web3;
            async move { source.into_source(web3).await.unwrap().pair_provider }
        })
        .collect()
        .await;

    let base_tokens = Arc::new(BaseTokens::new(
        eth.contracts().weth().address(),
        &args.shared.base_tokens,
    ));
    let mut allowed_tokens = args.allowed_tokens.clone();
    allowed_tokens.extend(base_tokens.tokens().iter().copied());
    allowed_tokens.push(model::order::BUY_ETH_ADDRESS);
    let unsupported_tokens = args.unsupported_tokens.clone();

    let finder = token_owner_finder::init(
        &args.token_owner_finder,
        web3.clone(),
        &chain,
        &http_factory,
        &pair_providers,
        vault.as_ref(),
        uniswapv3_factory.as_ref(),
        &base_tokens,
        eth.contracts().settlement().address(),
    )
    .await
    .expect("failed to initialize token owner finders");

    let trace_call_detector = args.tracing_node_url.as_ref().map(|tracing_node_url| {
        CachingDetector::new(
            Box::new(TraceCallDetector::new(
                shared::ethrpc::web3(
                    &args.shared.ethrpc,
                    &http_factory,
                    tracing_node_url,
                    "trace",
                ),
                eth.contracts().settlement().address(),
                finder,
            )),
            args.shared.token_quality_cache_expiry,
            args.shared.token_quality_cache_prefetch_time,
        )
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

    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Arc::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let block_retriever = args.shared.current_block.retriever(web3.clone());

    let code_fetcher = Arc::new(CachedCodeFetcher::new(Arc::new(web3.clone())));

    let mut price_estimator_factory = PriceEstimatorFactory::new(
        &args.price_estimation,
        &args.shared,
        factory::Network {
            web3: web3.clone(),
            simulation_web3,
            chain,
            native_token: eth.contracts().weth().address(),
            settlement: eth.contracts().settlement().address(),
            authenticator: eth
                .contracts()
                .settlement()
                .authenticator()
                .call()
                .await
                .expect("failed to query solver authenticator address"),
            base_tokens: base_tokens.clone(),
            block_stream: eth.current_block().clone(),
        },
        factory::Components {
            http_factory: http_factory.clone(),
            bad_token_detector: bad_token_detector.clone(),
            tokens: token_info_fetcher.clone(),
            code_fetcher: code_fetcher.clone(),
        },
    )
    .await
    .expect("failed to initialize price estimator factory");

    let native_price_estimator = price_estimator_factory
        .native_price_estimator(
            args.native_price_estimators.as_slice(),
            args.native_price_estimation_results_required,
            eth.contracts().weth().clone(),
        )
        .await
        .unwrap();
    let prices = db.fetch_latest_prices().await.unwrap();
    native_price_estimator.initialize_cache(prices).await;

    let price_estimator = price_estimator_factory
        .price_estimator(
            &args.order_quoting.price_estimation_drivers,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
        )
        .unwrap();

    let skip_event_sync_start = if args.skip_event_sync {
        block_number_to_block_number_hash(&web3, BlockNumber::Latest).await
    } else {
        None
    };

    let persistence =
        infra::persistence::Persistence::new(args.s3.into().unwrap(), Arc::new(db.clone())).await;
    let settlement_observer =
        crate::domain::settlement::Observer::new(eth.clone(), persistence.clone());
    let settlement_contract_start_index =
        if let Some(DeploymentInformation::BlockNumber(settlement_contract_start_index)) =
            eth.contracts().settlement().deployment_information()
        {
            settlement_contract_start_index
        } else {
            // If the deployment information can't be found, start from 0 (default
            // behaviour). For real contracts, the deployment information is specified
            // for all the networks, but it isn't specified for the e2e tests which deploy
            // the contracts from scratch
            tracing::warn!("Settlement contract deployment information not found");
            0
        };
    let settlement_event_indexer = EventUpdater::new(
        boundary::events::settlement::GPv2SettlementContract::new(
            eth.contracts().settlement().clone(),
        ),
        boundary::events::settlement::Indexer::new(
            db.clone(),
            settlement_observer,
            settlement_contract_start_index,
        ),
        block_retriever.clone(),
        skip_event_sync_start,
    );

    let archive_node_web3 = args.archive_node_url.as_ref().map_or(web3.clone(), |url| {
        boundary::web3_client(url, &args.shared.ethrpc)
    });

    let mut cow_amm_registry = cow_amm::Registry::new(archive_node_web3);
    for config in &args.cow_amm_configs {
        cow_amm_registry
            .add_listener(config.index_start, config.factory, config.helper)
            .await;
    }

    let quoter = Arc::new(OrderQuoter::new(
        price_estimator,
        native_price_estimator.clone(),
        gas_price_estimator,
        Arc::new(db.clone()),
        order_quoting::Validity {
            eip1271_onchain_quote: chrono::Duration::from_std(
                args.order_quoting.eip1271_onchain_quote_validity,
            )
            .unwrap(),
            presign_onchain_quote: chrono::Duration::from_std(
                args.order_quoting.presign_onchain_quote_validity,
            )
            .unwrap(),
            standard_quote: chrono::Duration::from_std(
                args.order_quoting.standard_offchain_quote_validity,
            )
            .unwrap(),
        },
        balance_fetcher.clone(),
        args.price_estimation.quote_verification,
    ));

    let solvable_orders_cache = SolvableOrdersCache::new(
        args.min_order_validity_period,
        persistence.clone(),
        infra::banned::Users::new(
            eth.contracts().chainalysis_oracle().clone(),
            args.banned_users,
        ),
        balance_fetcher.clone(),
        bad_token_detector.clone(),
        native_price_estimator.clone(),
        signature_validator.clone(),
        eth.contracts().weth().address(),
        args.limit_order_price_factor
            .try_into()
            .expect("limit order price factor can't be converted to BigDecimal"),
        domain::ProtocolFees::new(&args.fee_policies, args.fee_policy_max_partner_fee),
        cow_amm_registry.clone(),
        args.run_loop_native_price_timeout,
    );

    let liveness = Arc::new(Liveness::new(args.max_auction_age));
    observe::metrics::serve_metrics(liveness.clone(), args.metrics_address);

    let order_events_cleaner_config = crate::periodic_db_cleanup::OrderEventsCleanerConfig::new(
        args.order_events_cleanup_interval,
        args.order_events_cleanup_threshold,
    );
    let order_events_cleaner = crate::periodic_db_cleanup::OrderEventsCleaner::new(
        order_events_cleaner_config,
        db.clone(),
    );

    tokio::task::spawn(
        order_events_cleaner
            .run_forever()
            .instrument(tracing::info_span!("order_events_cleaner")),
    );

    let market_makable_token_list_configuration = TokenListConfiguration {
        url: args.trusted_tokens_url,
        update_interval: args.trusted_tokens_update_interval,
        chain_id,
        client: http_factory.create(),
        hardcoded: args.trusted_tokens.unwrap_or_default(),
    };
    // updated in background task
    let trusted_tokens =
        AutoUpdatingTokenList::from_configuration(market_makable_token_list_configuration).await;

    let mut maintenance = Maintenance::new(settlement_event_indexer, db.clone());
    maintenance.with_cow_amms(&cow_amm_registry);

    if !args.ethflow_contracts.is_empty() {
        let ethflow_refund_start_block = determine_ethflow_refund_indexing_start(
            &skip_event_sync_start,
            args.ethflow_indexing_start,
            &web3,
            chain_id,
            db.clone(),
        )
        .await;

        let refund_event_handler = EventUpdater::new_skip_blocks_before(
            // This cares only about ethflow refund events because all the other ethflow
            // events are already indexed by the OnchainOrderParser.
            EthFlowRefundRetriever::new(web3.clone(), args.ethflow_contracts.clone()),
            db.clone(),
            block_retriever.clone(),
            ethflow_refund_start_block,
        )
        .await
        .unwrap();

        let custom_ethflow_order_parser = EthFlowOnchainOrderParser {};
        let onchain_order_event_parser = OnchainOrderParser::new(
            db.clone(),
            web3.clone(),
            quoter.clone(),
            Box::new(custom_ethflow_order_parser),
            DomainSeparator::new(chain_id, eth.contracts().settlement().address()),
            eth.contracts().settlement().address(),
        );

        let ethflow_start_block = determine_ethflow_indexing_start(
            &skip_event_sync_start,
            args.ethflow_indexing_start,
            &web3,
            chain_id,
        )
        .await;

        let onchain_order_indexer = EventUpdater::new_skip_blocks_before(
            // The events from the ethflow contract are read with the more generic contract
            // interface called CoWSwapOnchainOrders.
            CoWSwapOnchainOrdersContract::new(web3.clone(), args.ethflow_contracts),
            onchain_order_event_parser,
            block_retriever,
            ethflow_start_block,
        )
        .await
        .expect("Should be able to initialize event updater. Database read issues?");

        maintenance.with_ethflow(onchain_order_indexer);
        // refunds are not critical for correctness and can therefore be indexed
        // sporadically in a background task
        let service_maintainer = ServiceMaintenance::new(vec![Arc::new(refund_event_handler)]);
        tokio::task::spawn(
            service_maintainer.run_maintenance_on_new_block(eth.current_block().clone()),
        );
    }

    let run_loop_config = run_loop::Config {
        submission_deadline: args.submission_deadline as u64,
        max_settlement_transaction_wait: args.max_settlement_transaction_wait,
        solve_deadline: args.solve_deadline,
        max_run_loop_delay: args.max_run_loop_delay,
        max_winners_per_auction: args.max_winners_per_auction,
        max_solutions_per_solver: args.max_solutions_per_solver,
    };

    let run = RunLoop::new(
        run_loop_config,
        eth,
        persistence.clone(),
        args.drivers
            .into_iter()
            .map(|driver| {
                Arc::new(infra::Driver::new(
                    driver.url,
                    driver.name,
                    driver.fairness_threshold.map(Into::into),
                ))
            })
            .collect(),
        solvable_orders_cache,
        trusted_tokens,
        liveness.clone(),
        Arc::new(maintenance),
    );
    run.run_forever().await;
}

async fn shadow_mode(args: Arguments) -> ! {
    let http_factory = HttpClientFactory::new(&args.http_client);

    let orderbook = infra::shadow::Orderbook::new(
        http_factory.create(),
        args.shadow.expect("missing shadow mode configuration"),
    );

    let drivers = args
        .drivers
        .into_iter()
        .map(|driver| {
            Arc::new(infra::Driver::new(
                driver.url,
                driver.name,
                driver.fairness_threshold.map(Into::into),
            ))
        })
        .collect();

    let trusted_tokens = {
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

        AutoUpdatingTokenList::from_configuration(TokenListConfiguration {
            url: args.trusted_tokens_url,
            update_interval: args.trusted_tokens_update_interval,
            chain_id,
            client: http_factory.create(),
            hardcoded: args.trusted_tokens.unwrap_or_default(),
        })
        .await
    };

    let liveness = Arc::new(Liveness::new(args.max_auction_age));
    observe::metrics::serve_metrics(liveness.clone(), args.metrics_address);

    let current_block = ethrpc::block_stream::current_block_stream(
        args.shared.node_url,
        args.shared.current_block.block_stream_poll_interval,
    )
    .await
    .expect("couldn't initialize current block stream");

    let shadow = shadow::RunLoop::new(
        orderbook,
        drivers,
        trusted_tokens,
        args.solve_deadline,
        liveness.clone(),
        current_block,
        args.max_winners_per_auction,
    );
    shadow.run_forever().await;
}
