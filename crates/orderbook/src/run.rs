use {
    crate::{
        api,
        arguments::Arguments,
        database::Postgres,
        ipfs::Ipfs,
        ipfs_app_data::IpfsAppData,
        orderbook::Orderbook,
        quoter::QuoteHandler,
    },
    anyhow::{anyhow, Context, Result},
    app_data::Validator,
    chain::Chain,
    clap::Parser,
    contracts::{BalancerV2Vault, GPv2Settlement, HooksTrampoline, IUniswapV3Factory, WETH9},
    ethcontract::errors::DeployError,
    futures::{FutureExt, StreamExt},
    model::{order::BUY_ETH_ADDRESS, DomainSeparator},
    observe::metrics::{serve_metrics, DEFAULT_METRICS_PORT},
    order_validation,
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
        gas_price::InstrumentedGasEstimator,
        http_client::HttpClientFactory,
        order_quoting::{self, OrderQuoter},
        order_validation::{OrderValidPeriodConfiguration, OrderValidator},
        price_estimation::{
            factory::{self, PriceEstimatorFactory},
            native::NativePriceEstimating,
            PriceEstimating,
            QuoteVerificationMode,
        },
        signature_validator,
        sources::{self, uniswap_v2::UniV2BaselineSourceParameters, BaselineSource},
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    },
    std::{future::Future, net::SocketAddr, sync::Arc, time::Duration},
    tokio::{task, task::JoinHandle},
    warp::Filter,
};

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    observe::tracing::initialize(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
    );
    tracing::info!("running order book with validated arguments:\n{}", args);
    observe::panic_hook::install();
    observe::metrics::setup_registry(Some("gp_v2_api".into()), None);
    run(args).await;
}

pub async fn run(args: Arguments) {
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

    let chain = Chain::try_from(chain_id).expect("incorrect chain ID");

    let signature_validator = signature_validator::validator(
        &web3,
        signature_validator::Contracts {
            settlement: settlement_contract.address(),
            vault_relayer,
        },
    );

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

    let hooks_contract = match args.hooks_contract_address {
        Some(address) => HooksTrampoline::at(&web3, address),
        None => HooksTrampoline::deployed(&web3)
            .await
            .expect("load hooks trampoline contract"),
    };

    verify_deployed_contract_constants(&settlement_contract, chain_id)
        .await
        .expect("Deployed contract constants don't match the ones in this binary");
    let domain_separator = DomainSeparator::new(chain_id, settlement_contract.address());
    let postgres = Postgres::new(args.db_url.as_str()).expect("failed to create database");

    let balance_fetcher = account_balances::fetcher(
        &web3,
        account_balances::Contracts {
            settlement: settlement_contract.address(),
            vault_relayer,
            vault: vault.as_ref().map(|contract| contract.address()),
        },
    );

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

    let baseline_sources = args
        .shared
        .baseline_sources
        .clone()
        .unwrap_or_else(|| sources::defaults_for_network(&chain));
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
        &chain,
        &http_factory,
        &pair_providers,
        vault.as_ref(),
        uniswapv3_factory.as_ref(),
        &base_tokens,
        settlement_contract.address(),
    )
    .await
    .expect("failed to initialize token owner finders");

    let trace_call_detector = args.tracing_node_url.as_ref().map(|tracing_node_url| {
        CachingDetector::new(
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

    let current_block_stream = args
        .shared
        .current_block
        .stream(args.shared.node_url.clone())
        .await
        .unwrap();

    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Arc::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));

    let code_fetcher = Arc::new(CachedCodeFetcher::new(Arc::new(web3.clone())));

    let mut price_estimator_factory = PriceEstimatorFactory::new(
        &args.price_estimation,
        &args.shared,
        factory::Network {
            web3: web3.clone(),
            simulation_web3,
            chain,
            native_token: native_token.address(),
            settlement: settlement_contract.address(),
            authenticator: settlement_contract
                .authenticator()
                .call()
                .await
                .expect("failed to query solver authenticator address"),
            base_tokens: base_tokens.clone(),
            block_stream: current_block_stream.clone(),
        },
        factory::Components {
            http_factory: http_factory.clone(),
            bad_token_detector: bad_token_detector.clone(),
            tokens: token_info_fetcher.clone(),
            code_fetcher: code_fetcher.clone(),
        },
    )
    .expect("failed to initialize price estimator factory");

    let native_price_estimator = price_estimator_factory
        .native_price_estimator(
            args.native_price_estimators.as_slice(),
            args.fast_price_estimation_results_required,
            native_token.clone(),
        )
        .await
        .unwrap();
    let prices = postgres.fetch_latest_prices().await.unwrap();
    native_price_estimator.initialize_cache(prices).await;

    let price_estimator = price_estimator_factory
        .price_estimator(
            &args.order_quoting.price_estimation_drivers,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
        )
        .unwrap();
    let fast_price_estimator = price_estimator_factory
        .fast_price_estimator(
            &args.order_quoting.price_estimation_drivers,
            args.fast_price_estimation_results_required,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
        )
        .unwrap();

    let validity_configuration = OrderValidPeriodConfiguration {
        min: args.min_order_validity_period,
        max_market: args.max_order_validity_period,
        max_limit: args.max_limit_order_validity_period,
    };

    let create_quoter = |price_estimator: Arc<dyn PriceEstimating>,
                         verification: QuoteVerificationMode| {
        Arc::new(OrderQuoter::new(
            price_estimator,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
            Arc::new(postgres.clone()),
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
            verification,
        ))
    };
    let optimal_quoter = create_quoter(price_estimator, args.price_estimation.quote_verification);
    // Fast quoting is able to return early and if none of the produced quotes are
    // verifiable we are left with no quote at all. Since fast estimates don't
    // make any promises on correctness we can just skip quote verification for
    // them.
    let fast_quoter = create_quoter(fast_price_estimator, QuoteVerificationMode::Unverified);

    let app_data_validator = Validator::new(args.app_data_size_limit);
    let chainalysis_oracle = contracts::ChainalysisOracle::deployed(&web3).await.ok();
    let order_validator = Arc::new(OrderValidator::new(
        Arc::new(order_validation::banned::Users::new(
            chainalysis_oracle,
            args.banned_users,
        )),
        validity_configuration,
        args.eip1271_skip_creation_validation,
        bad_token_detector.clone(),
        hooks_contract,
        optimal_quoter.clone(),
        balance_fetcher,
        signature_validator,
        Arc::new(postgres.clone()),
        args.max_limit_orders_per_user,
        code_fetcher,
        app_data_validator.clone(),
        args.max_gas_per_order,
    ));
    let ipfs = args
        .ipfs_gateway
        .map(|url| {
            Ipfs::new(
                http_factory.builder(),
                url,
                args.ipfs_pinata_auth
                    .map(|auth| format!("pinataGatewayToken={auth}")),
            )
        })
        .map(IpfsAppData::new);
    let app_data = Arc::new(crate::app_data::Registry::new(
        app_data_validator,
        postgres.clone(),
        ipfs,
    ));
    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        settlement_contract.address(),
        postgres.clone(),
        order_validator.clone(),
        app_data.clone(),
    ));

    check_database_connection(orderbook.as_ref()).await;
    let quotes = Arc::new(
        QuoteHandler::new(order_validator, optimal_quoter, app_data.clone())
            .with_fast_quoter(fast_quoter),
    );

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve_api = serve_api(
        postgres,
        orderbook.clone(),
        quotes,
        app_data,
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
        native_price_estimator,
    );

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
                Err(_) => panic!("API shutdown exceeded timeout"),
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
        .get_order(&Default::default())
        .await
        .expect("failed to connect to database");
}

#[allow(clippy::too_many_arguments)]
fn serve_api(
    database: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    app_data: Arc<crate::app_data::Registry>,
    address: SocketAddr,
    shutdown_receiver: impl Future<Output = ()> + Send + 'static,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
) -> JoinHandle<()> {
    let filter = api::handle_all_routes(
        database,
        orderbook,
        quotes,
        app_data,
        native_price_estimator,
    )
    .boxed();
    tracing::info!(%address, "serving order book");
    let warp_svc = warp::service(filter);
    let warp_svc = observe::make_service_with_task_local_storage!(warp_svc);
    let server = hyper::Server::bind(&address)
        .serve(warp_svc)
        .with_graceful_shutdown(shutdown_receiver)
        .map(|_| ());
    task::spawn(server)
}

/// Check that important constants such as the EIP 712 Domain Separator and
/// Order Type Hash used in this binary match the ones on the deployed
/// contract instance. Signature inconsistencies due to a mismatch of these
/// constants are hard to debug.
async fn verify_deployed_contract_constants(
    contract: &GPv2Settlement,
    chain_id: u64,
) -> Result<()> {
    let web3 = contract.raw_instance().web3();
    let bytecode = hex::encode(
        web3.eth()
            .code(contract.address(), None)
            .await
            .context("Could not load deployed bytecode")?
            .0,
    );

    let domain_separator = DomainSeparator::new(chain_id, contract.address());
    if !bytecode.contains(&hex::encode(domain_separator.0)) {
        return Err(anyhow!("Bytecode did not contain domain separator"));
    }

    if !bytecode.contains(&hex::encode(model::order::OrderData::TYPE_HASH)) {
        return Err(anyhow!("Bytecode did not contain order type hash"));
    }
    Ok(())
}
