use anyhow::{anyhow, Context, Result};
use contracts::{BalancerV2Vault, GPv2Settlement, WETH9};
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
    fee::EthAwareMinFeeCalculator,
    gas_price::InstrumentedGasEstimator,
    metrics::Metrics,
    orderbook::Orderbook,
    serve_api,
    solvable_orders::SolvableOrdersCache,
    verify_deployed_contract_constants,
};
use primitive_types::H160;
use shared::price_estimation::zeroex::ZeroExPriceEstimator;
use shared::zeroex_api::DefaultZeroExApi;
use shared::{
    bad_token::{
        cache::CachingDetector,
        list_based::{ListBasedDetector, UnknownTokenStrategy},
        trace_call::{
            AmmPairProviderFinder, BalancerVaultFinder, TokenOwnerFinding, TraceCallDetector,
        },
    },
    baseline_solver::BaseTokens,
    current_block::current_block_stream,
    maintenance::ServiceMaintenance,
    metrics::{serve_metrics, setup_metrics_registry, DEFAULT_METRICS_PORT},
    paraswap_api::DefaultParaswapApi,
    price_estimation::{
        baseline::BaselinePriceEstimator, paraswap::ParaswapPriceEstimator,
        priority::PriorityPriceEstimator, PriceEstimating, PriceEstimatorType,
    },
    recent_block_cache::CacheConfig,
    sources::{
        self,
        uniswap::{
            pool_cache::PoolCache,
            pool_fetching::{PoolFetcher, PoolFetching},
        },
        PoolAggregator,
    },
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    transport::create_instrumented_transport,
    transport::http::HttpTransport,
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use structopt::StructOpt;
use tokio::task;
use url::Url;

#[derive(Debug, StructOpt)]
struct Arguments {
    #[structopt(flatten)]
    shared: shared::arguments::Arguments,

    #[structopt(long, env, default_value = "0.0.0.0:8080")]
    bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[structopt(long, env, default_value = "postgresql://")]
    db_url: Url,

    /// Skip syncing past events (useful for local deployments)
    #[structopt(long)]
    skip_event_sync: bool,

    /// The minimum amount of time in seconds an order has to be valid for.
    #[structopt(
        long,
        env,
        default_value = "60",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_order_validity_period: Duration,

    /// Don't use the trace_callMany api that only some nodes support to check whether a token
    /// should be denied.
    /// Note that if a node does not support the api we still use the less accurate call api.
    #[structopt(long, env, parse(try_from_str), default_value = "false")]
    skip_trace_api: bool,

    /// The amount of time in seconds a classification of a token into good or bad is valid for.
    #[structopt(
        long,
        env,
        default_value = "600",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    token_quality_cache_expiry: Duration,

    /// List of token addresses to be ignored throughout service
    #[structopt(long, env, use_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    /// List of account addresses to be denied from order creation
    #[structopt(long, env, use_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// List of token addresses that should be allowed regardless of whether the bad token detector
    /// thinks they are bad. Base tokens are automatically allowed.
    #[structopt(long, env, use_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// The number of pairs that are automatically updated in the pool cache.
    #[structopt(long, env, default_value = "200")]
    pub pool_cache_lru_size: usize,

    /// Enable pre-sign orders. Pre-sign orders are accepted into the database without a valid
    /// signature, so this flag allows this feature to be turned off if malicious users are
    /// abusing the database by inserting a bunch of order rows that won't ever be valid.
    /// This flag can be removed once DDoS protection is implemented.
    #[structopt(long, env, parse(try_from_str), default_value = "false")]
    pub enable_presign_orders: bool,

    /// If solvable orders haven't been successfully update in this time in seconds attempting
    /// to get them errors and our liveness check fails.
    #[structopt(
        long,
        default_value = "300",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    solvable_orders_max_update_age: Duration,

    /// Gas Fee Factor: 1.0 means cost is forwarded to users alteration, 0.9 means there is a 10%
    /// subsidy, 1.1 means users pay 10% in fees than what we estimate we pay for gas.
    #[structopt(long, env, default_value = "1", parse(try_from_str = shared::arguments::parse_fee_factor))]
    pub fee_factor: f64,

    /// Used to specify additional fee subsidy factor based on app_ids contained in orders.
    /// Should take the form of a json string as shown in the following example:
    ///
    /// '0x0000000000000000000000000000000000000000000000000000000000000000:0.5,$PROJECT_APP_ID:0.7'
    ///
    /// Furthermore, a value of
    /// - 1 means no subsidy and is the default for all app_data not contained in this list.
    /// - 0.5 means that this project pays only 50% of the estimated fees.
    #[structopt(
        long,
        env,
        default_value = "",
        parse(try_from_str = parse_partner_fee_factor),
    )]
    partner_additional_fee_factors: HashMap<AppId, f64>,
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
    let args = Arguments::from_args();
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

    let native_token_price_estimation_amount = args
        .shared
        .amount_to_estimate_prices_with
        .or_else(|| shared::arguments::default_amount_to_estimate_prices_with(&network))
        .expect("No amount to estimate prices with set.");

    let vault = if BalancerV2Vault::raw_contract()
        .networks
        .contains_key(&network)
    {
        Some(
            BalancerV2Vault::deployed(&web3)
                .await
                .expect("couldn't load deployed vault contract"),
        )
    } else {
        // The Vault is not deployed on all networks we support, so allow it
        // to be `None` for those networks.
        tracing::warn!("No Balancer V2 Vault support for network {}", network);
        None
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
        vault,
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

    let pair_providers = sources::pair_providers(&args.shared.baseline_sources, chain_id, &web3)
        .await
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
            Arc::new(AmmPairProviderFinder {
                inner: provider.clone(),
                base_tokens: base_tokens.tokens().iter().copied().collect(),
            })
        })
        .collect();
    if let Some(finder) = BalancerVaultFinder::new(&web3).await.unwrap() {
        finders.push(Arc::new(finder));
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
    let pool_fetcher = Arc::new(
        PoolCache::new(
            CacheConfig {
                number_of_blocks_to_cache: args.shared.pool_cache_blocks,
                number_of_entries_to_auto_update: args.pool_cache_lru_size,
                maximum_recent_block_age: args.shared.pool_cache_maximum_recent_block_age,
                max_retries: args.shared.pool_cache_maximum_retries,
                delay_between_retries: args.shared.pool_cache_delay_between_retries_seconds,
            },
            Box::new(pool_aggregator),
            current_block_stream.clone(),
            metrics.clone(),
        )
        .expect("failed to create pool cache"),
    );
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: web3.clone(),
    })));
    let price_estimators = args
        .shared
        .price_estimators
        .iter()
        .map(|estimator| match estimator {
            PriceEstimatorType::Baseline => Box::new(BaselinePriceEstimator::new(
                pool_fetcher.clone(),
                gas_price_estimator.clone(),
                base_tokens.clone(),
                bad_token_detector.clone(),
                native_token.address(),
                native_token_price_estimation_amount,
            )) as Box<dyn PriceEstimating>,
            PriceEstimatorType::Paraswap => Box::new(ParaswapPriceEstimator {
                paraswap: Arc::new(DefaultParaswapApi {
                    client: client.clone(),
                    partner: args.shared.paraswap_partner.clone().unwrap_or_default(),
                }),
                token_info: token_info_fetcher.clone(),
                bad_token_detector: bad_token_detector.clone(),
                disabled_paraswap_dexs: args.shared.disabled_paraswap_dexs.clone(),
            }) as Box<dyn PriceEstimating>,
            PriceEstimatorType::ZeroEx => Box::new(ZeroExPriceEstimator {
                client: Arc::new(DefaultZeroExApi::with_default_url(client.clone())),
                bad_token_detector: bad_token_detector.clone(),
            }) as Box<dyn PriceEstimating>,
        })
        .collect::<Vec<_>>();
    let price_estimator = Arc::new(PriorityPriceEstimator::new(price_estimators));
    let fee_calculator = Arc::new(EthAwareMinFeeCalculator::new(
        price_estimator.clone(),
        gas_price_estimator,
        native_token.address(),
        database.clone(),
        args.fee_factor,
        bad_token_detector.clone(),
        args.partner_additional_fee_factors,
        native_token_price_estimation_amount,
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
    let service_maintainer = ServiceMaintenance {
        maintainers: vec![database.clone(), Arc::new(event_updater), pool_fetcher],
    };
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
    for pair_str in s.split(',').into_iter() {
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
