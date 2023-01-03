pub mod arguments;
pub mod ethflow_order;
pub mod refund_service;
pub mod submitter;

use contracts::CoWSwapEthFlow;
use ethcontract::{Account, PrivateKey};
use refund_service::RefundService;
use shared::{http_client::HttpClientFactory, metrics::LivenessChecking};
use sqlx::PgPool;
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

const SLEEP_TIME_BETWEEN_LOOPS: Duration = Duration::from_secs(30);
const SECONDS_FROM_LAST_LOOP_BEFORE_UNHEALTHY: Duration = Duration::from_secs(120);

pub async fn main(args: arguments::Arguments) {
    let pg_pool = PgPool::connect_lazy(args.db_url.as_str()).expect("failed to create database");

    let liveness = Arc::new(Liveness {
        // Program will be healthy at the start even if no loop was ran yet.
        last_successful_loop: RwLock::new(Instant::now()),
    });
    shared::metrics::serve_metrics(liveness.clone(), ([0, 0, 0, 0], args.metrics_port).into());

    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::ethrpc::web3(&args.ethrpc, &http_factory, &args.node_url, "base");
    let ethflow_contract = CoWSwapEthFlow::at(&web3, args.ethflow_contract);
    let refunder_account = Account::Offline(args.refunder_pk.parse::<PrivateKey>().unwrap(), None);
    let mut refunder = RefundService::new(
        pg_pool,
        web3,
        ethflow_contract,
        args.min_validity_duration.as_secs() as i64,
        args.min_slippage_bps,
        refunder_account,
    );
    loop {
        tracing::info!("Staring a new refunding loop");
        match refunder.try_to_refund_all_eligble_orders().await {
            Ok(_) => {
                track_refunding_loop_result("success");
                *liveness.last_successful_loop.write().expect(
                    "Lock is only written to at this point and the program should just crash on a panic",
                ) = Instant::now()
            }
            Err(err) => {
                track_refunding_loop_result("error");
                tracing::warn!("Error while refunding ethflow orders: {:?}", err)
            }
        }
        tokio::time::sleep(SLEEP_TIME_BETWEEN_LOOPS).await;
    }
}

struct Liveness {
    last_successful_loop: RwLock<Instant>,
}

#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        self.last_successful_loop
            .read()
            .ok()
            .map(|last_successful_loop| {
                Instant::now().duration_since(*last_successful_loop)
                    < SECONDS_FROM_LAST_LOOP_BEFORE_UNHEALTHY
            })
            .unwrap_or(false)
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
    Metrics::instance(global_metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
        .refunding_loops
        .with_label_values(&[result])
        .inc();
}
