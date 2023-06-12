/// Metrics for the driver.
#[derive(Debug, Clone, prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// Reasons for dropped solutions.
    #[metric(labels("solver", "reason"))]
    pub dropped_solutions: prometheus::IntCounterVec,
    /// The results of the solving process.
    #[metric(labels("solver", "result"))]
    pub solutions: prometheus::IntCounterVec,
    /// The results of the settlement process.
    #[metric(labels("solver", "result"))]
    pub settlements: prometheus::IntCounterVec,
    /// The results of the quoting process.
    #[metric(labels("solver", "result"))]
    pub quotes: prometheus::IntCounterVec,
}

/// Setup the metrics registry.
pub fn init() {
    global_metrics::setup_metrics_registry_reentrant(Some("driver".to_owned()), None);
}

/// Get the metrics instance.
pub fn get() -> &'static Metrics {
    Metrics::instance(global_metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
}
