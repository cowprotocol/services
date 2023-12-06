/// Metrics for the solver engine.
#[derive(Debug, Clone, prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// The amount of time this solver engine has for solving.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15))]
    pub time_limit: prometheus::Histogram,

    /// The amount of time this solver engine has left when it finished solving.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15))]
    pub remaining_time: prometheus::Histogram,

    /// Errors that occurred during solving.
    #[metric(labels("reason"))]
    pub solve_errors: prometheus::IntCounterVec,

    /// The number of solutions that were found.
    pub solutions: prometheus::IntCounter,
}

/// Setup the metrics registry.
pub fn init() {
    observe::metrics::setup_registry_reentrant(Some("solver-engine".to_owned()), None);
}

/// Get the metrics instance.
pub fn get() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
}
