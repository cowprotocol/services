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
        domain,
        event_updater::EventUpdater,
        infra,
        maintenance::Maintenance,
        run_loop::{self, RunLoop},
        shadow,
        shutdown_controller::ShutdownController,
        solvable_orders::SolvableOrdersCache,
    },
    alloy::{eips::BlockNumberOrTag, primitives::Address, providers::Provider},
    chain::Chain,
    clap::Parser,
    contracts::alloy::{BalancerV2Vault, GPv2Settlement, IUniswapV3Factory, WETH9},
    ethrpc::{Web3, block_stream::block_number_to_block_number_hash},
    futures::StreamExt,
    model::DomainSeparator,
    num::ToPrimitive,
    observe::metrics::LivenessChecking,
    shared::{
        account_balances::{self, BalanceSimulator},
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
        order_quoting::{self, OrderQuoter},
        price_estimation::factory::{self, PriceEstimatorFactory},
        signature_validator,
        sources::{BaselineSource, uniswap_v2::UniV2BaselineSourceParameters},
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
        token_list::{AutoUpdatingTokenList, TokenListConfiguration},
    },
    std::{
        sync::{Arc, RwLock, atomic::AtomicBool},
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
async fn ethrpc(url: &Url, ethrpc_args: &shared::web3::Arguments) -> infra::blockchain::Rpc {
    infra::blockchain::Rpc::new(url, ethrpc_args)
        .await
        .expect("connect ethereum RPC")
}

/// Creates unbuffered Web3 transport.
async fn unbuffered_ethrpc(url: &Url) -> infra::blockchain::Rpc {
    ethrpc(
        url,
        &shared::web3::Arguments {
            ethrpc_max_batch_size: 0,
            ethrpc_max_concurrent_requests: 0,
            ethrpc_batch_delay: Default::default(),
        },
    )
    .await
}

#[instrument(skip_all)]
async fn ethereum(
    web3: Web3,
    unbuffered_web3: Web3,
    chain: &Chain,
    url: Url,
    contracts: infra::blockchain::contracts::Addresses,
    current_block_args: &shared::current_block::Arguments,
) -> infra::Ethereum {
    infra::Ethereum::new(
        web3,
        unbuffered_web3,
        chain,
        url,
        contracts,
        current_block_args,
    )
    .await
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
    #[cfg(unix)]
    observe::heap_dump_handler::spawn_heap_dump_handler();

    let commit_hash = option_env!("VERGEN_GIT_SHA").unwrap_or("COMMIT_INFO_NOT_FOUND");

    tracing::info!(%commit_hash, "running autopilot with validated arguments:\n{}", args);

    observe::metrics::setup_registry(Some("gp_v2_autopilot".into()), None);

    if args.drivers.is_empty() {
        panic!("colocation is enabled but no drivers are configured");
    }

    if args.shadow.is_some() {
        shadow_mode(args).await;
    } else {
        run(args, ShutdownController::default()).await;
    }
}

/// Assumes tracing and metrics registry have already been set up.
pub async fn run(args: Arguments, shutdown_controller: ShutdownController) {
    assert!(args.shadow.is_none(), "cannot run in shadow mode");
    let db_write = Postgres::new(
        args.db_write_url.as_str(),
        crate::database::Config {
            insert_batch_size: args.insert_batch_size,
            max_pool_size: args.database_pool.db_max_connections,
        },
    )
    .await
    .unwrap();

    // If the DB is in read-only mode, running ANALYZE is not possible and will
    // trigger and error https://www.postgresql.org/docs/current/hot-standby.html
    crate::database::run_database_metrics_work(db_write.clone());

    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::web3::web3(&args.shared.ethrpc, &args.shared.node_url, "base");
    let simulation_web3 = args
        .shared
        .simulation_node_url
        .as_ref()
        .map(|node_url| shared::web3::web3(&args.shared.ethrpc, node_url, "simulation"));

    let chain_id = web3
        .provider
        .get_chain_id()
        .instrument(info_span!("chain_id"))
        .await
        .expect("Could not get chainId");
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
        signatures: args.shared.signatures_contract_address,
        weth: args.shared.native_token_address,
        balances: args.shared.balances_contract_address,
        trampoline: args.shared.hooks_contract_address,
    };
    let eth = ethereum(
        web3.clone(),
        unbuffered_ethrpc.web3().clone(),
        &chain,
        url,
        contracts.clone(),
        &args.shared.current_block,
    )
    .await;

    let vault_relayer = eth
        .contracts()
        .settlement()
        .vaultRelayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");

    let vault_address = args.shared.balancer_v2_vault_address.or_else(|| {
        let chain_id = chain.id();
        let addr = BalancerV2Vault::deployment_address(&chain_id);
        if addr.is_none() {
            tracing::warn!(
                chain_id,
                "balancer contracts are not deployed on this network"
            );
        }
        addr
    });
    let vault =
        vault_address.map(|address| BalancerV2Vault::Instance::new(address, web3.provider.clone()));

    let uniswapv3_factory = IUniswapV3Factory::Instance::deployed(&web3.provider)
        .instrument(info_span!("uniswapv3_deployed"))
        .await
        .inspect_err(|err| tracing::warn!(%err, "error while fetching IUniswapV3Factory instance"))
        .ok();

    let chain = Chain::try_from(chain_id).expect("incorrect chain ID");

    let balance_overrider = args.price_estimation.balance_overrides.init(web3.clone());
    let signature_validator = signature_validator::validator(
        &web3,
        signature_validator::Contracts {
            settlement: eth.contracts().settlement().clone(),
            signatures: eth.contracts().signatures().clone(),
            vault_relayer,
        },
        balance_overrider.clone(),
    );

    let balance_fetcher = account_balances::cached(
        &web3,
        BalanceSimulator::new(
            eth.contracts().settlement().clone(),
            eth.contracts().balances().clone(),
            vault_relayer,
            vault_address,
            balance_overrider,
        ),
        eth.current_block().clone(),
    );

    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            &http_factory,
            &web3,
            args.shared.gas_estimators.as_slice(),
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
        *eth.contracts().weth().address(),
        &args.shared.base_tokens,
    ));
    let mut allowed_tokens = args.allowed_tokens.clone();
    allowed_tokens.extend(base_tokens.tokens().iter());
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
        *eth.contracts().settlement().address(),
    )
    .instrument(info_span!("token_owner_finder_init"))
    .await
    .expect("failed to initialize token owner finders");

    let trace_call_detector = args.tracing_node_url.as_ref().map(|tracing_node_url| {
        CachingDetector::new(
            Box::new(TraceCallDetector::new(
                shared::web3::web3(&args.shared.ethrpc, tracing_node_url, "trace"),
                *eth.contracts().settlement().address(),
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
    let block_retriever = Arc::new(web3.provider.clone());

    let code_fetcher = Arc::new(CachedCodeFetcher::new(Arc::new(web3.clone())));

    let mut price_estimator_factory = PriceEstimatorFactory::new(
        &args.price_estimation,
        &args.shared,
        factory::Network {
            web3: web3.clone(),
            simulation_web3,
            chain,
            settlement: *eth.contracts().settlement().address(),
            native_token: *eth.contracts().weth().address(),
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
    let prices = db_write.fetch_latest_prices().await.unwrap();
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
            block_number_to_block_number_hash(&web3.provider, BlockNumberOrTag::Latest)
                .await
                .expect("Failed to fetch latest block"),
        )
    } else {
        None
    };

    let persistence =
        infra::persistence::Persistence::new(args.s3.into().unwrap(), Arc::new(db_write.clone()))
            .instrument(info_span!("persistence_init"))
            .await;
    let settlement_contract_start_index = match GPv2Settlement::deployment_block(&chain_id) {
        Some(block) => {
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
            web3.provider.clone(),
            *eth.contracts().settlement().address(),
        ),
        boundary::events::settlement::Indexer::new(
            db_write.clone(),
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
            .add_listener(
                config.index_start,
                config.factory,
                config.helper,
                db_write.pool.clone(),
            )
            .await;
    }

    let quoter = Arc::new(OrderQuoter::new(
        price_estimator,
        native_price_estimator.clone(),
        gas_price_estimator,
        Arc::new(db_write.clone()),
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
            args.banned_users_max_cache_size.get().to_u64().unwrap(),
        ),
        balance_fetcher.clone(),
        bad_token_detector.clone(),
        native_price_estimator.clone(),
        signature_validator.clone(),
        *eth.contracts().weth().address(),
        args.limit_order_price_factor
            .try_into()
            .expect("limit order price factor can't be converted to BigDecimal"),
        domain::ProtocolFees::new(
            &args.fee_policies_config,
            args.shared.volume_fee_bucket_overrides.clone(),
            args.shared.enable_sell_equals_buy_volume_fee,
        ),
        cow_amm_registry.clone(),
        args.run_loop_native_price_timeout,
        *eth.contracts().settlement().address(),
        args.disable_order_balance_filter,
        args.disable_1271_order_sig_filter,
        args.disable_1271_order_balance_filter,
    );

    let liveness = Arc::new(Liveness::new(args.max_auction_age));
    let startup = Arc::new(Some(AtomicBool::new(false)));

    let (api_shutdown_sender, api_shutdown_receiver) = tokio::sync::oneshot::channel();
    let api_task = tokio::spawn(infra::api::serve(
        args.api_address,
        native_price_estimator.clone(),
        args.price_estimation.quote_timeout,
        api_shutdown_receiver,
    ));

    observe::metrics::serve_metrics(
        liveness.clone(),
        args.metrics_address,
        Default::default(),
        startup.clone(),
    );

    let order_events_cleaner_config = crate::periodic_db_cleanup::OrderEventsCleanerConfig::new(
        args.order_events_cleanup_interval,
        args.order_events_cleanup_threshold,
    );
    let order_events_cleaner = crate::periodic_db_cleanup::OrderEventsCleaner::new(
        order_events_cleaner_config,
        db_write.clone(),
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
    let settlement_observer =
        crate::domain::settlement::Observer::new(eth.clone(), persistence.clone());

    let mut maintenance = Maintenance::new(
        settlement_event_indexer,
        db_write.clone(),
        settlement_observer,
    );
    maintenance.add_cow_amm_indexer(&cow_amm_registry);

    if !args.ethflow_contracts.is_empty() {
        let ethflow_refund_start_block = determine_ethflow_refund_indexing_start(
            &skip_event_sync_start,
            args.ethflow_indexing_start,
            &web3,
            chain_id,
            db_write.clone(),
        )
        .await;

        let refund_event_handler = EventUpdater::new_skip_blocks_before(
            // This cares only about ethflow refund events because all the other ethflow
            // events are already indexed by the OnchainOrderParser.
            EthFlowRefundRetriever::new(web3.clone(), args.ethflow_contracts.clone()),
            db_write.clone(),
            block_retriever.clone(),
            ethflow_refund_start_block,
        )
        .instrument(info_span!("refund_event_handler_init"))
        .await
        .unwrap();

        let custom_ethflow_order_parser = EthFlowOnchainOrderParser {};
        let onchain_order_event_parser = OnchainOrderParser::new(
            db_write.clone(),
            web3.clone(),
            quoter.clone(),
            Box::new(custom_ethflow_order_parser),
            DomainSeparator::new(chain_id, *eth.contracts().settlement().address()),
            *eth.contracts().settlement().address(),
            eth.contracts().trampoline().clone(),
        );

        let ethflow_start_block = determine_ethflow_indexing_start(
            &skip_event_sync_start,
            args.ethflow_indexing_start,
            &web3,
            chain_id,
            &db_write,
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

        maintenance.add_ethflow_indexing(onchain_order_indexer, refund_event_handler);
    }

    let run_loop_config = run_loop::Config {
        submission_deadline: args.submission_deadline as u64,
        max_settlement_transaction_wait: args.max_settlement_transaction_wait,
        solve_deadline: args.solve_deadline,
        max_run_loop_delay: args.max_run_loop_delay,
        max_winners_per_auction: args.max_winners_per_auction,
        max_solutions_per_solver: args.max_solutions_per_solver,
        enable_leader_lock: args.enable_leader_lock,
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

    let awaiter = maintenance
        .spawn_maintenance_task(eth.current_block().clone(), args.max_maintenance_timeout);

    let run = RunLoop::new(
        run_loop_config,
        eth,
        persistence.clone(),
        drivers,
        solvable_orders_cache,
        trusted_tokens,
        run_loop::Probes {
            liveness: liveness.clone(),
            startup,
        },
        awaiter,
    );
    run.run_forever(shutdown_controller).await;

    api_shutdown_sender.send(()).ok();
    api_task.await.ok();
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
                Account::Address(Address::random()),
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

    let web3 = shared::web3::web3(&args.shared.ethrpc, &args.shared.node_url, "base");
    let weth = WETH9::Instance::deployed(&web3.provider)
        .await
        .expect("couldn't find deployed WETH contract");

    let trusted_tokens = {
        let chain_id = web3
            .provider
            .get_chain_id()
            .await
            .expect("Could not get chainId");
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
    observe::metrics::serve_metrics(
        liveness.clone(),
        args.metrics_address,
        Default::default(),
        Default::default(),
    );

    let current_block = args
        .shared
        .current_block
        .stream(args.shared.node_url, web3.provider.clone())
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
        (*weth.address()).into(),
    );
    shadow.run_forever().await;
}
