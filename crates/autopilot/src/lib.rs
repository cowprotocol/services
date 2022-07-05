pub mod arguments;

use shared::metrics::LivenessChecking;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Number of seconds program has been running for.
    seconds_alive: prometheus::IntGauge,
}

struct Liveness;
#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        true
    }
}

/// Assumes tracing and metrics registry have already been set up.
pub async fn main(args: arguments::Arguments) {
    let update_metrics = async {
        let start = Instant::now();
        let metrics = Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap();
        loop {
            metrics.seconds_alive.set(start.elapsed().as_secs() as i64);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };
    let serve_metrics = shared::metrics::serve_metrics(Arc::new(Liveness), args.metrics_address);
    tokio::select! {
        result = serve_metrics => tracing::error!(?result, "serve_metrics exited"),
        _ = update_metrics => (),
    };
}
