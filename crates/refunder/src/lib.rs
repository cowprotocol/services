pub mod arguments;
pub mod ethflow_order;
pub mod refund_service;
pub mod submitter;

use {
    crate::arguments::Arguments,
    clap::Parser,
    contracts::CoWSwapEthFlow,
    ethcontract::{Account, PrivateKey},
    refund_service::RefundService,
    shared::{http_client::HttpClientFactory, metrics::LivenessChecking},
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
    observe::tracing::initialize(
        args.logging.log_filter.as_str(),
        args.logging.log_stderr_threshold,
    );
    observe::panic_hook::install();
    tracing::info!("running refunder with validated arguments:\n{}", args);
    observe::metrics::setup_registry(Some("refunder".into()), None);
    run(args).await;
}

pub async fn run(args: arguments::Arguments) {
    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::ethrpc::web3(&args.ethrpc, &http_factory, &args.node_url, "base");
    if let Some(expected_chain_id) = args.chain_id {
        let chain_id = web3
            .eth()
            .chain_id()
            .await
            .expect("Could not get chainId")
            .as_u64();
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
    shared::metrics::serve_metrics(liveness.clone(), ([0, 0, 0, 0], args.metrics_port).into());

    let ethflow_contract = CoWSwapEthFlow::at(&web3, args.ethflow_contract);
    let refunder_account = Account::Offline(args.refunder_pk.parse::<PrivateKey>().unwrap(), None);
    let mut refunder = RefundService::new(
        pg_pool,
        web3,
        ethflow_contract,
        i64::try_from(args.min_validity_duration.as_secs()).unwrap_or(i64::MAX),
        args.min_slippage_bps,
        refunder_account,
    );
    loop {
        tracing::info!("Staring a new refunding loop");
        match refunder.try_to_refund_all_eligble_orders().await {
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
