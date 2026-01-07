pub mod arguments;
pub mod refund_service;
pub mod submitter;

// Re-export commonly used types for external consumers (e.g., e2e tests)
pub use refund_service::RefundStatus;
use {
    crate::arguments::Arguments,
    alloy::{providers::Provider, signers::local::PrivateKeySigner},
    clap::Parser,
    contracts::alloy::CoWSwapEthFlow,
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
    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::ethrpc::web3(&args.ethrpc, &http_factory, &args.node_url, "base");
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

    let pg_pool = PgPool::connect_lazy(args.db_url.as_str()).expect("failed to create database");

    let liveness = Arc::new(Liveness {
        // Program will be healthy at the start even if no loop was ran yet.
        last_successful_loop: RwLock::new(Instant::now()),
    });
    observe::metrics::serve_metrics(
        liveness.clone(),
        ([0, 0, 0, 0], args.metrics_port).into(),
        Default::default(),
        Default::default(),
    );

    let ethflow_contracts = args
        .ethflow_contracts
        .iter()
        .map(|contract| CoWSwapEthFlow::Instance::new(*contract, web3.alloy.clone()))
        .collect();
    let refunder_account = Box::new(
        args.refunder_pk
            .parse::<PrivateKeySigner>()
            .expect("couldn't parse refunder private key"),
    );
    let mut refunder = RefundService::new(
        pg_pool,
        web3,
        ethflow_contracts,
        i64::try_from(args.min_validity_duration.as_secs()).unwrap_or(i64::MAX),
        args.min_price_deviation_bps,
        refunder_account,
        args.max_gas_price,
        args.start_priority_fee_tip,
    );
    loop {
        tracing::info!("Staring a new refunding loop");
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
    /// Tracks the result of every refunding loops.
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
