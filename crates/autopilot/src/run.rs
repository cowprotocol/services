use {
    crate::{
        arguments::{Account, Arguments},
        boundary,
        database::{
            Postgres,
            ethflow_events::event_retriever::EthFlowRefundRetriever,
            onchain_order_events::{
                OnchainOrderParser,
                ethflow_events::{
                    EthFlowOnchainOrderParser,
                    determine_ethflow_indexing_start,
                    determine_ethflow_refund_indexing_start,
                },
                event_retriever::CoWSwapOnchainOrdersContract,
            },
        },
        domain::{self, competition::SolverParticipationGuard},
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
    ethcontract::{
        BlockNumber,
        H160,
        common::DeploymentInformation,
        dyns::DynWeb3,
        errors::DeployError,
    },
    ethrpc::block_stream::block_number_to_block_number_hash,
    futures::stream::StreamExt,
    model::DomainSeparator,
    observe::metrics::LivenessChecking,
    shared::{
        account_balances,
        arguments::tracing_config,
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
        sources::{BaselineSource, uniswap_v2::UniV2BaselineSourceParameters},
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
        token_list::{AutoUpdatingTokenList, TokenListConfiguration},
    },
    std::{
        sync::{Arc, RwLock},
        time::{Duration, Instant},
    },
    tracing::{Instrument, info_span, instrument},
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

/// Creates Web3 transport based on the given config.
#[instrument(skip_all)]
async fn ethrpc(url: &Url, ethrpc_args: &shared::ethrpc::Arguments) -> infra::blockchain::Rpc {
    infra::blockchain::Rpc::new(url, ethrpc_args)
        .await
        .expect("connect ethereum RPC")
}

/// Creates unbuffered Web3 transport.
async fn unbuffered_ethrpc(url: &Url) -> infra::blockchain::Rpc {
    ethrpc(
        url,
        &shared::ethrpc::Arguments {
            ethrpc_max_batch_size: 0,
            ethrpc_max_concurrent_requests: 0,
            ethrpc_batch_delay: Default::default(),
        },
    )
    .await
}

#[instrument(skip_all, fields(chain = ?chain))]
async fn ethereum(
    web3: DynWeb3,
    unbuffered_web3: DynWeb3,
    chain: &Chain,
    url: Url,
    contracts: infra::blockchain::contracts::Addresses,
    poll_interval: Duration,
) -> infra::Ethereum {
    infra::Ethereum::new(web3, unbuffered_web3, chain, url, contracts, poll_interval).await
}

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    let obs_config = observe::Config::new(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
        args.shared.logging.use_json_logs,
        tracing_config(&args.shared.tracing, "autopilot".into()),
    );
    observe::tracing::initialize(&obs_config);
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
    // Start a new span that measures the initialization phase of the autopilot
    let startup_span = info_span!("autopilot_startup", ?args.shared.node_url);
    let startup_span = startup_span.enter();

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
        .instrument(info_span!("chain_id"))
        .await
        .expect("Could not get chainId")
        .as_u64();
    if let Some(expected_chain_id) = args.shared.chain_id {
        assert_eq!(
            chain_id, expected_chain_id,
            "connected to node with incorrect chain ID",
        );
    }

    let unbuffered_ethrpc = unbuffered_ethrpc(&args.shared.node_url).await;
    let ethrpc = ethrpc(&args.shared.node_url, &args.shared.ethrpc).await;
    let chain = ethrpc.chain();
    let web3 = ethrpc.web3().clone();
    let url = ethrpc.url().clone();
    let contracts = infra::blockchain::contracts::Addresses {
        settlement: args.shared.settlement_contract_address,
        weth: args.shared.native_token_address,
        trampoline: args.shared.hooks_contract_address,
    };
    let eth = ethereum(
        web3.clone(),
        unbuffered_ethrpc.web3().clone(),
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
        .instrument(info_span!("vault_relayer_call"))
        .await
        .expect("Couldn't get vault relayer address");
    let vault = match args.shared.balancer_v2_vault_address {
        Some(address) => Some(contracts::BalancerV2Vault::with_deployment_info(
            &web3, address, None,
        )),
        None => match BalancerV2Vault::deployed(&web3)
            .instrument(info_span!("balancerV2vault_deployed"))
            .await
        {
            Ok(contract) => Some(contract),
            Err(DeployError::NotFound(_)) => {
                tracing::warn!("balancer contracts are not deployed on this network");
                None
            }
            Err(err) => panic!("failed to get balancer vault contract: {err}"),
        },
    };
    let uniswapv3_factory = match IUniswapV3Factory::deployed(&web3)
        .instrument(info_span!("uniswapv3_deployed"))
        .await
    {
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
        .instrument(info_span!("pair_providers"))
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
    .instrument(info_span!("token_owner_finder_init"))
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
                .instrument(info_span!("authenticator_call"))
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
    .instrument(info_span!("price_estimator_factory"))
    .await
    .expect("failed to initialize price estimator factory");

    let native_price_estimator = price_estimator_factory
        .native_price_estimator(
            args.native_price_estimators.as_slice(),
            args.native_price_estimation_results_required,
            eth.contracts().weth().clone(),
        )
        .instrument(info_span!("native_price_estimator"))
        .await
        .unwrap();
    let prices = db.fetch_latest_prices().await.unwrap();
    native_price_estimator.initialize_cache(prices);

    let price_estimator = price_estimator_factory
        .price_estimator(
            &args
                .order_quoting
                .price_estimation_drivers
                .iter()
                .map(|price_estimator_driver| price_estimator_driver.clone().into())
                .collect::<Vec<_>>(),
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
        )
        .unwrap();

    let skip_event_sync_start = if args.skip_event_sync {
        Some(
            block_number_to_block_number_hash(&web3, BlockNumber::Latest)
                .await
                .expect("Failed to fetch latest block"),
        )
    } else {
        None
    };

    let (competition_updates_sender, competition_updates_receiver) =
        tokio::sync::mpsc::unbounded_channel();

    let persistence =
        infra::persistence::Persistence::new(args.s3.into().unwrap(), Arc::new(db.clone()))
            .instrument(info_span!("persistence_init"))
            .await;
    let settlement_observer =
        crate::domain::settlement::Observer::new(eth.clone(), persistence.clone());
    let settlement_contract_start_index = match contracts::GPv2Settlement::raw_contract()
        .networks
        .get(&chain_id.to_string())
        .and_then(|v| v.deployment_information)
    {
        Some(DeploymentInformation::BlockNumber(block)) => {
            tracing::debug!(block, "found settlement contract deployment");
            block
        }
        _ => {
            // If the deployment information can't be found, start from 0 (default
            // behaviour). For real contracts, the deployment information is specified
            // for all the networks, but it isn't specified for the e2e tests which deploy
            // the contracts from scratch
            tracing::warn!("Settlement contract deployment information not found");
            0
        }
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
        args.price_estimation.quote_timeout,
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
        eth.contracts().settlement().address(),
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
        .instrument(info_span!("refund_event_handler_init"))
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
            eth.contracts().trampoline().clone(),
        );

        let ethflow_start_block = determine_ethflow_indexing_start(
            &skip_event_sync_start,
            args.ethflow_indexing_start,
            &web3,
            chain_id,
            &db,
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
        .instrument(info_span!("onchain_order_indexer_init"))
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

    let drivers_futures = args
        .drivers
        .into_iter()
        .map(|driver| async move {
            infra::Driver::try_new(
                driver.url,
                driver.name.clone(),
                driver.fairness_threshold.map(Into::into),
                driver.submission_account,
                driver.requested_timeout_on_problems,
            )
            .await
            .map(Arc::new)
            .expect("failed to load solver configuration")
        })
        .collect::<Vec<_>>();

    let drivers: Vec<_> = futures::future::join_all(drivers_futures)
        .instrument(info_span!("drivers_init"))
        .await
        .into_iter()
        .collect();

    let solver_participation_guard = SolverParticipationGuard::new(
        eth.clone(),
        persistence.clone(),
        competition_updates_receiver,
        args.db_based_solver_participation_guard,
        drivers.iter().cloned(),
    );

    let run = RunLoop::new(
        run_loop_config,
        eth,
        persistence.clone(),
        drivers,
        solver_participation_guard,
        solvable_orders_cache,
        trusted_tokens,
        liveness.clone(),
        Arc::new(maintenance),
        competition_updates_sender,
    );
    drop(startup_span);
    run.run_forever().await;
}

async fn shadow_mode(args: Arguments) -> ! {
    let http_factory = HttpClientFactory::new(&args.http_client);

    let orderbook = infra::shadow::Orderbook::new(
        http_factory.create(),
        args.shadow.expect("missing shadow mode configuration"),
    );

    let drivers_futures = args
        .drivers
        .into_iter()
        .map(|driver| async move {
            infra::Driver::try_new(
                driver.url,
                driver.name.clone(),
                driver.fairness_threshold.map(Into::into),
                // HACK: the auction logic expects all drivers
                // to use a different submission address. But
                // in the shadow environment all drivers use
                // the same address to avoid creating new keys
                // before a solver is actually ready.
                // Luckily the shadow autopilot doesn't use
                // this address for anything important so we
                // can simply generate random addresses here.
                Account::Address(H160::random()),
                driver.requested_timeout_on_problems,
            )
            .await
            .map(Arc::new)
            .expect("failed to load solver configuration")
        })
        .collect::<Vec<_>>();

    let drivers = futures::future::join_all(drivers_futures)
        .await
        .into_iter()
        .collect();

    let web3 = shared::ethrpc::web3(
        &args.shared.ethrpc,
        &http_factory,
        &args.shared.node_url,
        "base",
    );
    let weth = contracts::WETH9::deployed(&web3)
        .await
        .expect("couldn't find deployed WETH contract");

    let trusted_tokens = {
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
        weth.address().into(),
    );
    shadow.run_forever().await;
}
