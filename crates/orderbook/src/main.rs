use anyhow::{anyhow, Context, Result};
use clap::{ArgEnum, Parser};
use contracts::{BalancerV2Vault, GPv2Settlement, WETH9};
use ethcontract::errors::DeployError;
use model::{
    app_id::AppId,
    order::{OrderUid, BUY_ETH_ADDRESS},
    DomainSeparator,
};
use orderbook::{
    account_balances::Web3BalanceFetcher,
    api::{order_validation::OrderValidator, post_quote::OrderQuoter},
    database::{self, orders::OrderFilter, Postgres},
    event_updater::EventUpdater,
    fee::{EthAwareMinFeeCalculator, FeeSubsidyConfiguration},
    gas_price::InstrumentedGasEstimator,
    metrics::Metrics,
    orderbook::Orderbook,
    serve_api,
    solvable_orders::SolvableOrdersCache,
    verify_deployed_contract_constants,
};
use primitive_types::H160;
use shared::oneinch_api::OneInchClientImpl;
use shared::price_estimation::native::NativePriceEstimator;
use shared::price_estimation::oneinch::OneInchPriceEstimator;
use shared::price_estimation::quasimodo::QuasimodoPriceEstimator;
use shared::price_estimation::sanitized::SanitizedPriceEstimator;
use shared::price_estimation::zeroex::ZeroExPriceEstimator;
use shared::zeroex_api::DefaultZeroExApi;
use shared::{
    bad_token::{
        cache::CachingDetector,
        list_based::{ListBasedDetector, UnknownTokenStrategy},
        trace_call::{
            BalancerVaultFinder, TokenOwnerFinding, TraceCallDetector,
            UniswapLikePairProviderFinder,
        },
    },
    baseline_solver::BaseTokens,
    current_block::current_block_stream,
    maintenance::ServiceMaintenance,
    metrics::{serve_metrics, setup_metrics_registry, DEFAULT_METRICS_PORT},
    paraswap_api::DefaultParaswapApi,
    price_estimation::{
        baseline::BaselinePriceEstimator, competition::CompetitionPriceEstimator,
        instrumented::InstrumentedPriceEstimator, paraswap::ParaswapPriceEstimator,
        PriceEstimating, PriceEstimatorType,
    },
    recent_block_cache::CacheConfig,
    sources::{
        self,
        uniswap_v2::{
            pool_cache::PoolCache,
            pool_fetching::{PoolFetcher, PoolFetching},
        },
        PoolAggregator,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    transport::create_instrumented_transport,
    transport::http::HttpTransport,
};
use shared::{
    http_solver::{DefaultHttpSolverApi, SolverConfig},
    network::network_name,
    price_estimation::cached::CachingPriceEstimator,
    sources::balancer_v2::BalancerFactoryKind,
    sources::{
        balancer_v2::{pool_fetching::BalancerContracts, BalancerPoolFetcher},
        BaselineSource,
    },
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::task;
use url::Url;

#[derive(Debug, Parser)]
struct Arguments {
    #[clap(flatten)]
    shared: shared::arguments::Arguments,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[clap(long, env, default_value = "postgresql://")]
    db_url: Url,

    /// Skip syncing past events (useful for local deployments)
    #[clap(long)]
    skip_event_sync: bool,

    /// The minimum amount of time in seconds an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "60",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_order_validity_period: Duration,

    /// Don't use the trace_callMany api that only some nodes support to check whether a token
    /// should be denied.
    /// Note that if a node does not support the api we still use the less accurate call api.
    #[clap(long, env, parse(try_from_str), default_value = "false")]
    skip_trace_api: bool,

    /// The amount of time in seconds a classification of a token into good or bad is valid for.
    #[clap(
        long,
        env,
        default_value = "600",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    token_quality_cache_expiry: Duration,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// List of token addresses that should be allowed regardless of whether the bad token detector
    /// thinks they are bad. Base tokens are automatically allowed.
    #[clap(long, env, use_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: usize,

    /// Enable pre-sign orders. Pre-sign orders are accepted into the database without a valid
    /// signature, so this flag allows this feature to be turned off if malicious users are
    /// abusing the database by inserting a bunch of order rows that won't ever be valid.
    /// This flag can be removed once DDoS protection is implemented.
    #[clap(long, env, parse(try_from_str), default_value = "false")]
    pub enable_presign_orders: bool,

    /// If solvable orders haven't been successfully update in this time in seconds attempting
    /// to get them errors and our liveness check fails.
    #[clap(
        long,
        default_value = "300",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    solvable_orders_max_update_age: Duration,

    /// A flat fee discount denominated in the network's native token (i.e. Ether for Mainnet).
    ///
    /// Note that flat fee discounts are applied BEFORE any multiplicative factors from either
    /// `--fee-factor` or `--partner-additional-fee-factors` configuration.
    #[clap(long, env, default_value = "0")]
    fee_discount: f64,

    /// The minimum value for the discounted fee in the network's native token (i.e. Ether for
    /// Mainnet).
    ///
    /// Note that this minimum is applied BEFORE any multiplicative factors from either
    /// `--fee-factor` or `--partner-additional-fee-factors` configuration.
    #[clap(long, env, default_value = "0")]
    min_discounted_fee: f64,

    /// Gas Fee Factor: 1.0 means cost is forwarded to users alteration, 0.9 means there is a 10%
    /// subsidy, 1.1 means users pay 10% in fees than what we estimate we pay for gas.
    #[clap(long, env, default_value = "1", parse(try_from_str = shared::arguments::parse_unbounded_factor))]
    fee_factor: f64,

    /// Used to specify additional fee subsidy factor based on app_ids contained in orders.
    /// Should take the form of a json string as shown in the following example:
    ///
    /// '0x0000000000000000000000000000000000000000000000000000000000000000:0.5,$PROJECT_APP_ID:0.7'
    ///
    /// Furthermore, a value of
    /// - 1 means no subsidy and is the default for all app_data not contained in this list.
    /// - 0.5 means that this project pays only 50% of the estimated fees.
    #[clap(
        long,
        env,
        default_value = "",
        parse(try_from_str = parse_partner_fee_factor),
    )]
    partner_additional_fee_factors: HashMap<AppId, f64>,

    /// The API endpoint to call the mip v2 solver for price estimation
    #[clap(long, env)]
    quasimodo_solver_url: Option<Url>,

    /// The maximum time price estimates will be cached for.
    #[clap(
        long,
        default_value = "10",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    price_estimator_cache_max_age_secs: Duration,

    /// The maximum number of price estimates that will be cached.
    #[clap(long, default_value = "1000")]
    price_estimator_cache_size: usize,

    /// The list of disabled 1Inch protocols. By default, the `PMM1` protocol
    /// (representing a private market maker) is disabled as it seems to
    /// produce invalid swaps.
    #[clap(long, env, default_value = "PMM1", use_delimiter = true)]
    disabled_one_inch_protocols: Vec<String>,

    /// The 1Inch REST API URL to use.
    #[structopt(long, env, default_value = "https://api.1inch.exchange/")]
    one_inch_url: Url,
}

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
    let args = Arguments::parse();
    shared::tracing::initialize(
        args.shared.log_filter.as_str(),
        args.shared.log_stderr_threshold,
    );
    tracing::info!("running order book with validated {:#?}", args);

    setup_metrics_registry(Some("gp_v2_api".into()), None);
    let metrics = Arc::new(Metrics::new().unwrap());

    let client = shared::http_client(args.shared.http_timeout);

    let transport = create_instrumented_transport(
        HttpTransport::new(client.clone(), args.shared.node_url.clone(), "".to_string()),
        metrics.clone(),
    );
    let web3 = web3::Web3::new(transport);
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
        .shared
        .amount_to_estimate_prices_with
        .or_else(|| shared::arguments::default_amount_to_estimate_prices_with(&network))
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

    let event_updater = EventUpdater::new(
        settlement_contract.clone(),
        database.as_ref().clone(),
        sync_start,
    );
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
    let pair_providers = sources::pair_providers(&web3, &baseline_sources)
        .await
        .expect("failed to load baseline source pair providers")
        .values()
        .cloned()
        .collect::<Vec<_>>();

    let base_tokens = Arc::new(BaseTokens::new(
        native_token.address(),
        &args.shared.base_tokens,
    ));
    let mut allowed_tokens = args.allowed_tokens.clone();
    allowed_tokens.extend(base_tokens.tokens().iter().copied());
    allowed_tokens.push(BUY_ETH_ADDRESS);
    let unsupported_tokens = args.unsupported_tokens.clone();

    let mut finders: Vec<Arc<dyn TokenOwnerFinding>> = pair_providers
        .iter()
        .map(|provider| -> Arc<dyn TokenOwnerFinding> {
            Arc::new(UniswapLikePairProviderFinder {
                inner: provider.clone(),
                base_tokens: base_tokens.tokens().iter().copied().collect(),
            })
        })
        .collect();
    if let Some(contract) = &vault {
        finders.push(Arc::new(BalancerVaultFinder(contract.clone())));
    }
    let trace_call_detector = TraceCallDetector {
        web3: web3.clone(),
        finders,
        base_tokens: base_tokens.tokens().clone(),
        settlement_contract: settlement_contract.address(),
    };
    let caching_detector = CachingDetector::new(
        Box::new(trace_call_detector),
        args.token_quality_cache_expiry,
    );
    let bad_token_detector = Arc::new(ListBasedDetector::new(
        allowed_tokens,
        unsupported_tokens,
        if args.skip_trace_api {
            UnknownTokenStrategy::Allow
        } else {
            UnknownTokenStrategy::Forward(Box::new(caching_detector))
        },
    ));

    let current_block_stream =
        current_block_stream(web3.clone(), args.shared.block_stream_poll_interval_seconds)
            .await
            .unwrap();

    let pool_aggregator = PoolAggregator {
        pool_fetchers: pair_providers
            .into_iter()
            .map(|pair_provider| {
                Arc::new(PoolFetcher {
                    pair_provider,
                    web3: web3.clone(),
                }) as Arc<dyn PoolFetching>
            })
            .collect(),
    };

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
            Box::new(pool_aggregator),
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
    let one_inch_api = Arc::new(
        OneInchClientImpl::new(args.one_inch_url.clone(), client.clone(), chain_id).unwrap(),
    );
    let instrumented = |inner: Box<dyn PriceEstimating>, name: &str| {
        InstrumentedPriceEstimator::new(inner, name.to_string(), metrics.clone())
    };
    let instrumented_and_cached =
        |inner: Box<dyn PriceEstimating>, name: &str, cache_age: Duration, cache_size: usize| {
            // Instrument first then cache so instrumented is close to inner estimator.
            CachingPriceEstimator::new(
                Box::new(instrumented(inner, name)),
                cache_age,
                cache_size,
                metrics.clone(),
                name.to_string(),
            )
        };
    let create_base_estimator = |&estimator| -> Box<dyn PriceEstimating> {
        match estimator {
            PriceEstimatorType::Baseline => Box::new(BaselinePriceEstimator::new(
                pool_fetcher.clone(),
                gas_price_estimator.clone(),
                base_tokens.clone(),
                native_token.address(),
                native_token_price_estimation_amount,
            )),
            PriceEstimatorType::Paraswap => Box::new(ParaswapPriceEstimator {
                paraswap: Arc::new(DefaultParaswapApi {
                    client: client.clone(),
                    partner: args.shared.paraswap_partner.clone().unwrap_or_default(),
                }),
                token_info: token_info_fetcher.clone(),
                disabled_paraswap_dexs: args.shared.disabled_paraswap_dexs.clone(),
            }),
            PriceEstimatorType::ZeroEx => Box::new(ZeroExPriceEstimator {
                api: zeroex_api.clone(),
            }),
            PriceEstimatorType::Quasimodo => Box::new(QuasimodoPriceEstimator {
                api: Arc::new(DefaultHttpSolverApi {
                    name: "quasimodo-price-estimator",
                    network_name: network_name.to_string(),
                    chain_id,
                    base: args.quasimodo_solver_url.clone().expect(
                        "quasimodo solver url is required when using quasimodo price estimation",
                    ),
                    client: client.clone(),
                    config: SolverConfig {
                        api_key: None,
                        max_nr_exec_orders: 100,
                        has_ucp_policy_parameter: false,
                        use_internal_buffers: args.shared.quasimodo_uses_internal_buffers.into(),
                    },
                }),
                pools: pool_fetcher.clone(),
                balancer_pools: balancer_pool_fetcher.clone(),
                token_info: token_info_fetcher.clone(),
                gas_info: gas_price_estimator.clone(),
                native_token: native_token.address(),
                base_tokens: base_tokens.clone(),
            }),
            PriceEstimatorType::OneInch => Box::new(OneInchPriceEstimator::new(
                one_inch_api.clone(),
                args.disabled_one_inch_protocols.clone(),
            )),
        }
    };

    let price_estimators = args
        .shared
        .price_estimators
        .iter()
        .map(|estimator| -> (String, Box<dyn PriceEstimating>) {
            let base_estimator = create_base_estimator(estimator);
            (
                estimator.name(),
                if let PriceEstimatorType::Baseline = estimator {
                    Box::new(instrumented(base_estimator, &estimator.name()))
                } else {
                    Box::new(instrumented_and_cached(
                        base_estimator,
                        &estimator.name(),
                        args.price_estimator_cache_max_age_secs,
                        args.price_estimator_cache_size,
                    ))
                },
            )
        })
        .collect();

    let price_estimator = Arc::new(SanitizedPriceEstimator::new(
        CompetitionPriceEstimator::new(price_estimators),
        native_token.address(),
        bad_token_detector.clone(),
    ));
    let native_price_estimator = Arc::new(NativePriceEstimator::new(
        price_estimator.clone(),
        native_token.address(),
        native_token_price_estimation_amount,
    ));
    let fee_calculator = Arc::new(EthAwareMinFeeCalculator::new(
        price_estimator.clone(),
        gas_price_estimator,
        native_token.address(),
        database.clone(),
        bad_token_detector.clone(),
        FeeSubsidyConfiguration {
            fee_discount: args.fee_discount,
            min_discounted_fee: args.min_discounted_fee,
            fee_factor: args.fee_factor,
            partner_additional_fee_factors: args.partner_additional_fee_factors,
        },
        native_price_estimator,
    ));

    let solvable_orders_cache = SolvableOrdersCache::new(
        args.min_order_validity_period,
        database.clone(),
        balance_fetcher.clone(),
        bad_token_detector.clone(),
        current_block_stream.clone(),
    );
    let block = current_block_stream.borrow().number.unwrap().as_u64();
    solvable_orders_cache
        .update(block)
        .await
        .expect("failed to perform initial solvable orders update");
    let order_validator = Arc::new(OrderValidator::new(
        Box::new(web3.clone()),
        native_token.clone(),
        args.banned_users,
        args.min_order_validity_period,
        fee_calculator.clone(),
        bad_token_detector.clone(),
        balance_fetcher,
    ));
    let orderbook = Arc::new(Orderbook::new(
        domain_separator,
        settlement_contract.address(),
        database.clone(),
        bad_token_detector,
        args.enable_presign_orders,
        solvable_orders_cache,
        args.solvable_orders_max_update_age,
        order_validator.clone(),
    ));
    let mut service_maintainer = ServiceMaintenance {
        maintainers: vec![database.clone(), Arc::new(event_updater), pool_fetcher],
    };
    if let Some(balancer) = balancer_pool_fetcher {
        service_maintainer.maintainers.push(balancer);
    }
    check_database_connection(orderbook.as_ref()).await;
    let quoter = Arc::new(OrderQuoter::new(
        fee_calculator,
        price_estimator,
        order_validator,
    ));
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve_api = serve_api(
        database.clone(),
        orderbook.clone(),
        quoter,
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
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

/// Parses a comma separated list of colon separated values representing fee factors for AppIds.
fn parse_partner_fee_factor(s: &str) -> Result<HashMap<AppId, f64>> {
    let mut res = HashMap::default();
    if s.is_empty() {
        return Ok(res);
    }
    for pair_str in s.split(',') {
        let mut split = pair_str.trim().split(':');
        let key = split
            .next()
            .ok_or_else(|| anyhow!("missing AppId"))?
            .trim()
            .parse()
            .context("failed to parse address")?;
        let value = split
            .next()
            .ok_or_else(|| anyhow!("missing value"))?
            .trim()
            .parse::<f64>()
            .context("failed to parse fee factor")?;
        if split.next().is_some() {
            return Err(anyhow!("Invalid pair lengths"));
        }
        res.insert(key, value);
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;

    #[test]
    fn parse_partner_fee_factor_ok() {
        let x = "0x0000000000000000000000000000000000000000000000000000000000000000";
        let y = "0x0101010101010101010101010101010101010101010101010101010101010101";
        // without spaces
        assert_eq!(
            parse_partner_fee_factor(&format!("{}:0.5,{}:0.7", x, y)).unwrap(),
            hashmap! { AppId([0u8; 32]) => 0.5, AppId([1u8; 32]) => 0.7 }
        );
        // with spaces
        assert_eq!(
            parse_partner_fee_factor(&format!("{}: 0.5, {}: 0.7", x, y)).unwrap(),
            hashmap! { AppId([0u8; 32]) => 0.5, AppId([1u8; 32]) => 0.7 }
        );
        // whole numbers
        assert_eq!(
            parse_partner_fee_factor(&format!("{}: 1, {}: 2", x, y)).unwrap(),
            hashmap! { AppId([0u8; 32]) => 1., AppId([1u8; 32]) => 2. }
        );
    }

    #[test]
    fn parse_partner_fee_factor_err() {
        assert!(parse_partner_fee_factor("0x1:0.5,0x2:0.7").is_err());
        assert!(parse_partner_fee_factor("0x12:0.5,0x22:0.7").is_err());
        assert!(parse_partner_fee_factor(
            "0x0000000000000000000000000000000000000000000000000000000000000000:0.5:3"
        )
        .is_err());
        assert!(parse_partner_fee_factor(
            "0x0000000000000000000000000000000000000000000000000000000000000000:word"
        )
        .is_err());
    }

    #[test]
    fn parse_partner_fee_factor_ok_on_empty() {
        assert!(parse_partner_fee_factor("").unwrap().is_empty());
    }
}
