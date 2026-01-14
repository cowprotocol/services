//! Automated refunder for expired EthFlow orders.
//!
//! Monitors EthFlow orders and returns ETH to users whose orders expired
//! without filling. Runs every 30 seconds: queries database, validates on-chain
//! status, submits batch refunds.
//!
//! Shares PostgreSQL with orderbook (read-only). Refunds tracked via on-chain
//! events.

pub mod arguments;
pub mod infra;
pub mod refund_service;
pub mod submitter;
pub mod traits;

// Re-export commonly used types for external consumers (e.g., e2e tests)
pub use traits::RefundStatus;
use {
    crate::{
        arguments::Arguments,
        infra::{AlloyChain, Postgres},
        submitter::Submitter,
    },
    alloy::{providers::Provider, signers::local::PrivateKeySigner},
    clap::Parser,
    observe::metrics::LivenessChecking,
    refund_service::RefundService,
    shared::http_client::HttpClientFactory,
    sqlx::PgPool,
    std::{
        sync::{Arc, RwLock},
        time::{Duration, Instant},
    },
};

const LOOP_INTERVAL: Duration = Duration::from_secs(30);
const DELAY_FROM_LAST_LOOP_BEFORE_UNHEALTHY: Duration = LOOP_INTERVAL.saturating_mul(4);

pub async fn start(args: impl Iterator<Item = String>) {
    let args = Arguments::parse_from(args);
    let obs_config = observe::Config::new(
        args.logging.log_filter.as_str(),
        args.logging.log_stderr_threshold,
        args.logging.use_json_logs,
        None,
    );
    observe::tracing::initialize(&obs_config);
    observe::panic_hook::install();
    #[cfg(unix)]
    observe::heap_dump_handler::spawn_heap_dump_handler();
    tracing::info!("running refunder with validated arguments:\n{}", args);
    observe::metrics::setup_registry(Some("refunder".into()), None);
    run(args).await;
}

pub async fn run(args: arguments::Arguments) {
    // Observability setup
    let liveness = Arc::new(Liveness {
        last_successful_loop: RwLock::new(Instant::now()),
    });
    observe::metrics::serve_metrics(
        liveness.clone(),
        ([0, 0, 0, 0], args.metrics_port).into(),
        Default::default(),
        Default::default(),
    );

    // Database initialization
    let pg_pool = PgPool::connect_lazy(args.db_url.as_str()).expect("failed to create database");
    let database = Postgres::new(pg_pool);

    // Blockchain/RPC setup
    let web3 = shared::ethrpc::web3(
        &args.ethrpc,
        &HttpClientFactory::new(&args.http_client),
        &args.node_url,
        "base",
    );

    if let Some(expected_chain_id) = args.chain_id {
        let chain_id = web3
            .alloy
            .get_chain_id()
            .await
            .expect("Could not get chainId");
        assert_eq!(
            chain_id, expected_chain_id,
            "connected to node with incorrect chain ID",
        );
    }

    let chain = AlloyChain::new(web3.alloy.clone(), args.ethflow_contracts.clone());

    // Signer/wallet configuration
    let refunder_account = Box::new(
        args.refunder_pk
            .parse::<PrivateKeySigner>()
            .expect("couldn't parse refunder private key"),
    );
    let signer_address = refunder_account.address();
    let gas_estimator = Box::new(web3.legacy.clone());
    web3.wallet.register_signer(refunder_account);

    // Transaction submitter
    let submitter = Submitter {
        web3,
        signer_address,
        gas_estimator,
        gas_parameters_of_last_tx: None,
        nonce_of_last_submission: None,
        max_gas_price: args.max_gas_price,
        start_priority_fee_tip: args.start_priority_fee_tip,
    };

    // Service construction
    let min_validity_duration =
        i64::try_from(args.min_validity_duration.as_secs()).unwrap_or(i64::MAX);
    let min_price_deviation = args.min_price_deviation_bps as f64 / 10000f64;

    let mut refunder = RefundService::new(
        database,
        chain,
        submitter,
        min_validity_duration,
        min_price_deviation,
    );

    // Main loop
    loop {
        tracing::info!("Starting a new refunding loop");
        match refunder.try_to_refund_all_eligible_orders().await {
            Ok(_) => {
                track_refunding_loop_result("success");
                *liveness.last_successful_loop.write().unwrap() = Instant::now()
            }
            Err(err) => {
                track_refunding_loop_result("error");
                tracing::warn!("Error while refunding ethflow orders: {:?}", err)
            }
        }
        tokio::time::sleep(LOOP_INTERVAL).await;
    }
}

struct Liveness {
    last_successful_loop: RwLock<Instant>,
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        Instant::now().duration_since(*self.last_successful_loop.read().unwrap())
            < DELAY_FROM_LAST_LOOP_BEFORE_UNHEALTHY
    }
}

#[derive(prometheus_metric_storage::MetricStorage, Debug)]
#[metric(subsystem = "main")]
struct Metrics {
    /// Refunding loop outcomes.
    #[metric(labels("result"))]
    refunding_loops: prometheus::IntCounterVec,
}

fn track_refunding_loop_result(result: &str) {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
        .refunding_loops
        .with_label_values(&[result])
        .inc();
}
