use {
    crate::{
        api,
        arguments::Arguments,
        config::Configuration,
        database::Postgres,
        ipfs::Ipfs,
        ipfs_app_data::IpfsAppData,
        orderbook::Orderbook,
        quoter::QuoteHandler,
    },
    alloy::providers::Provider,
    anyhow::{Context, Result, anyhow},
    app_data::Validator,
    chain::Chain,
    clap::Parser,
    contracts::alloy::{
        BalancerV2Vault,
        ChainalysisOracle,
        GPv2Settlement,
        HooksTrampoline,
        WETH9,
        support::Balances,
    },
    model::DomainSeparator,
    num::ToPrimitive,
    observe::metrics::{DEFAULT_METRICS_PORT, serve_metrics},
    order_validation,
    shared::{
        account_balances::{self, BalanceSimulator},
        arguments::tracing_config,
        bad_token::list_based::DenyListedTokens,
        code_fetching::CachedCodeFetcher,
        gas_price::InstrumentedGasEstimator,
        http_client::HttpClientFactory,
        order_quoting::{self, OrderQuoter},
        order_validation::{OrderValidPeriodConfiguration, OrderValidator},
        price_estimation::{
            PriceEstimating,
            QuoteVerificationMode,
            factory::{self, PriceEstimatorFactory},
            native::{FallbackNativePriceEstimator, NativePriceEstimating},
        },
        signature_validator,
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    },
    std::{future::Future, net::SocketAddr, sync::Arc, time::Duration},
    tokio::task::{self, JoinHandle},
};

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    let obs_config = observe::Config::new(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
        args.shared.logging.use_json_logs,
        tracing_config(&args.shared.tracing, "orderbook".into()),
    );
    observe::tracing::init::initialize(&obs_config);
    tracing::info!("running order book with validated arguments:\n{}", args);
    observe::panic_hook::install();
    observe::metrics::setup_registry(Some("gp_v2_api".into()), None);
    #[cfg(unix)]
    observe::heap_dump_handler::spawn_heap_dump_handler();
    let config = Configuration::from_path(&args.config)
        .await
        .expect("failed to load configuration file");
    run(args, config).await;
}

pub async fn run(args: Arguments, config: Configuration) {
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
        .await
        .expect("Could not get chainId");
    if let Some(expected_chain_id) = args.shared.chain_id {
        assert_eq!(
            chain_id, expected_chain_id,
            "connected to node with incorrect chain ID",
        );
    }

    let settlement_contract = match args.shared.settlement_contract_address {
        Some(address) => GPv2Settlement::Instance::new(address, web3.provider.clone()),
        None => GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .expect("load settlement contract"),
    };
    let balances_contract = match args.shared.balances_contract_address {
        Some(address) => Balances::Instance::new(address, web3.provider.clone()),
        None => Balances::Instance::deployed(&web3.provider.clone())
            .await
            .expect("load balances contract"),
    };
    let vault_relayer = settlement_contract
        .vaultRelayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");
    let signatures_contract = match args.shared.signatures_contract_address {
        Some(address) => {
            contracts::alloy::support::Signatures::Instance::new(address, web3.provider.clone())
        }
        None => contracts::alloy::support::Signatures::Instance::deployed(&web3.provider)
            .await
            .expect("load signatures contract"),
    };
    let native_token = match args.shared.native_token_address {
        Some(address) => WETH9::Instance::new(address, web3.provider.clone()),
        None => WETH9::Instance::deployed(&web3.provider)
            .await
            .expect("load native token contract"),
    };

    let chain = Chain::try_from(chain_id).expect("incorrect chain ID");

    let balance_overrider = args.price_estimation.balance_overrides.init(web3.clone());
    let signature_validator = signature_validator::validator(
        &web3,
        signature_validator::Contracts {
            settlement: settlement_contract.clone(),
            signatures: signatures_contract,
            vault_relayer,
        },
        balance_overrider.clone(),
    );

    let vault_address = args.shared.balancer_v2_vault_address.or_else(|| {
        let chain_id = chain.id();
        match BalancerV2Vault::deployment_address(&chain_id) {
            addr @ Some(_) => addr,
            addr @ None => {
                tracing::warn!(
                    chain_id,
                    "balancer contracts are not deployed on this network"
                );
                addr
            }
        }
    });

    let hooks_contract = match args.shared.hooks_contract_address {
        Some(address) => HooksTrampoline::Instance::new(address, web3.provider.clone()),
        None => HooksTrampoline::Instance::deployed(&web3.provider)
            .await
            .expect("load hooks trampoline contract"),
    };

    verify_deployed_contract_constants(&settlement_contract, chain_id)
        .await
        .expect("Deployed contract constants don't match the ones in this binary");
    let domain_separator = DomainSeparator::new(chain_id, *settlement_contract.address());
    let db_config = crate::database::Config {
        max_pool_size: args.database_pool.db_max_connections.get(),
    };
    let postgres_write = Postgres::try_new(args.db_write_url.as_str(), db_config.clone())
        .expect("failed to create database");

    let postgres_read = if let Some(db_read_url) = args.db_read_url
        && args.db_write_url != db_read_url
    {
        Postgres::try_new(db_read_url.as_str(), db_config)
            .expect("failed to create read replica database")
    } else {
        postgres_write.clone()
    };

    let balance_fetcher = account_balances::fetcher(
        &web3,
        BalanceSimulator::new(
            settlement_contract.clone(),
            balances_contract.clone(),
            vault_relayer,
            vault_address,
            balance_overrider,
        ),
    );

    let gas_price_estimator = Arc::new(InstrumentedGasEstimator::new(
        shared::gas_price_estimation::create_priority_estimator(
            &http_factory,
            &web3,
            args.shared.gas_estimators.as_slice(),
        )
        .await
        .expect("failed to create gas price estimator"),
    ));

    let deny_listed_tokens = DenyListedTokens::new(config.unsupported_tokens);

    let current_block_stream = args
        .shared
        .current_block
        .stream(args.shared.node_url.clone(), web3.provider.clone())
        .await
        .unwrap();

    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Arc::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));

    let code_fetcher = Arc::new(CachedCodeFetcher::new(Arc::new(web3.clone())));

    let mut price_estimator_factory = PriceEstimatorFactory::new(
        &args.price_estimation,
        &args.shared,
        &config.native_price_estimation.shared,
        factory::Network {
            web3: web3.clone(),
            simulation_web3,
            chain,
            settlement: *settlement_contract.address(),
            native_token: *native_token.address(),
            authenticator: settlement_contract
                .authenticator()
                .call()
                .await
                .expect("failed to query solver authenticator address"),
            block_stream: current_block_stream.clone(),
        },
        factory::Components {
            http_factory: http_factory.clone(),
            deny_listed_tokens: deny_listed_tokens.clone(),
            tokens: token_info_fetcher.clone(),
            code_fetcher: code_fetcher.clone(),
        },
    )
    .await
    .expect("failed to initialize price estimator factory");

    let prices = postgres_write.fetch_latest_prices().await.unwrap();
    let cache = shared::price_estimation::native_price_cache::Cache::new(
        config.native_price_estimation.shared.cache.max_age,
        prices,
    );
    let primary = price_estimator_factory
        .native_price_estimator(
            config.native_price_estimation.estimators.as_slice(),
            config.native_price_estimation.results_required,
            &native_token,
        )
        .await
        .expect("failed to build primary native price estimator");

    let inner: Box<dyn NativePriceEstimating> =
        if let Some(ref fallback_config) = config.native_price_estimation.fallback_estimators {
            let fallback = price_estimator_factory
                .native_price_estimator(
                    fallback_config.as_slice(),
                    config.native_price_estimation.results_required,
                    &native_token,
                )
                .await
                .expect("failed to build fallback native price estimator");
            Box::new(FallbackNativePriceEstimator::new(primary, fallback))
        } else {
            primary
        };

    let native_price_estimator: Arc<dyn NativePriceEstimating> = Arc::new(
        price_estimator_factory
            .caching_native_price_estimator_from_inner(inner, cache)
            .await,
    );

    let price_estimator = price_estimator_factory
        .price_estimator(
            &args
                .order_quoting
                .price_estimation_drivers
                .iter()
                .map(|price_estimator| price_estimator.clone().into())
                .collect::<Vec<_>>(),
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
        )
        .unwrap();
    let fast_price_estimator = price_estimator_factory
        .fast_price_estimator(
            &args
                .order_quoting
                .price_estimation_drivers
                .iter()
                .map(|price_estimator| price_estimator.clone().into())
                .collect::<Vec<_>>(),
            config.native_price_estimation.results_required,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
        )
        .unwrap();

    let validity_configuration = OrderValidPeriodConfiguration {
        min: config.order_validation.min_order_validity_period,
        max_market: config.order_validation.max_order_validity_period,
        max_limit: config.order_validation.max_limit_order_validity_period,
    };

    let create_quoter = |price_estimator: Arc<dyn PriceEstimating>,
                         verification: QuoteVerificationMode| {
        Arc::new(OrderQuoter::new(
            price_estimator,
            native_price_estimator.clone(),
            gas_price_estimator.clone(),
            Arc::new(postgres_write.clone()),
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
            args.price_estimation.quote_timeout,
        ))
    };
    let optimal_quoter = create_quoter(price_estimator, args.price_estimation.quote_verification);
    // Fast quoting is able to return early and if none of the produced quotes are
    // verifiable we are left with no quote at all. Since fast estimates don't
    // make any promises on correctness we can just skip quote verification for
    // them.
    let fast_quoter = create_quoter(fast_price_estimator, QuoteVerificationMode::Unverified);

    let app_data_validator = Validator::new(config.app_data_size_limit);
    let chainalysis_oracle = ChainalysisOracle::Instance::deployed(&web3.provider)
        .await
        .ok();
    let order_validator = Arc::new(OrderValidator::new(
        native_token,
        Arc::new(order_validation::banned::Users::new(
            chainalysis_oracle,
            config.banned_users.addresses,
            config.banned_users.max_cache_size.get().to_u64().unwrap(),
        )),
        validity_configuration,
        config.eip1271_skip_creation_validation,
        deny_listed_tokens.clone(),
        hooks_contract,
        optimal_quoter.clone(),
        balance_fetcher,
        signature_validator,
        Arc::new(postgres_write.clone()),
        config.order_validation.max_limit_orders_per_user,
        code_fetcher,
        app_data_validator.clone(),
        config.order_validation.max_gas_per_order,
        config.order_validation.same_tokens_policy,
    ));
    let ipfs = config
        .ipfs
        .map(|ipfs| {
            let pinata_query = ipfs
                .auth_token
                .map(|auth| format!("pinataGatewayToken={auth}"));
            Ipfs::new(http_factory.builder(), ipfs.gateway, pinata_query)
        })
        .map(IpfsAppData::new);
    let app_data = Arc::new(crate::app_data::Registry::new(
        app_data_validator,
        postgres_write.clone(),
        ipfs,
    ));
    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        *settlement_contract.address(),
        postgres_write.clone(),
        postgres_read.clone(),
        order_validator.clone(),
        app_data.clone(),
        config.active_order_competition_threshold,
    ));

    check_database_connection(orderbook.as_ref()).await;
    let quotes = QuoteHandler::new(
        order_validator,
        optimal_quoter,
        app_data.clone(),
        config.volume_fee,
        args.shared.volume_fee_bucket_overrides.clone(),
        args.shared.enable_sell_equals_buy_volume_fee,
    )
    .with_fast_quoter(fast_quoter);

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve_api = serve_api(
        postgres_write,
        postgres_read,
        orderbook.clone(),
        quotes,
        app_data,
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
        native_price_estimator,
        args.price_estimation.quote_timeout,
    );

    let mut metrics_address = args.bind_address;
    metrics_address.set_port(DEFAULT_METRICS_PORT);
    tracing::info!(%metrics_address, "serving metrics");
    let metrics_task = serve_metrics(
        orderbook,
        metrics_address,
        Default::default(),
        Default::default(),
    );

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

#[expect(clippy::too_many_arguments)]
fn serve_api(
    database: Postgres,
    database_replica: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: QuoteHandler,
    app_data: Arc<crate::app_data::Registry>,
    address: SocketAddr,
    shutdown_receiver: impl Future<Output = ()> + Send + 'static,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    quote_timeout: Duration,
) -> JoinHandle<()> {
    let app = api::handle_all_routes(
        database,
        database_replica,
        orderbook,
        quotes,
        app_data,
        native_price_estimator,
        quote_timeout,
    );
    tracing::info!(%address, "serving order book");

    task::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => listener,
            Err(err) => {
                tracing::error!(?err, "failed to bind server");
                return;
            }
        };
        if let Err(err) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_receiver)
            .await
        {
            tracing::error!(?err, "server error");
        }
    })
}

/// Check that important constants such as the EIP 712 Domain Separator and
/// Order Type Hash used in this binary match the ones on the deployed
/// contract instance. Signature inconsistencies due to a mismatch of these
/// constants are hard to debug.
async fn verify_deployed_contract_constants(
    contract: &GPv2Settlement::Instance,
    chain_id: u64,
) -> Result<()> {
    let provider = contract.provider();
    let bytecode = const_hex::encode(
        provider
            .get_code_at(*contract.address())
            .await
            .context("Could not load deployed bytecode")?
            .0,
    );

    let domain_separator = DomainSeparator::new(chain_id, *contract.address());
    if !bytecode.contains(&const_hex::encode(domain_separator.0)) {
        return Err(anyhow!("Bytecode did not contain domain separator"));
    }

    if !bytecode.contains(&const_hex::encode(model::order::OrderData::TYPE_HASH)) {
        return Err(anyhow!("Bytecode did not contain order type hash"));
    }
    Ok(())
}
