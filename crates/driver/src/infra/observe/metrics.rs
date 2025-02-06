/// Metrics for the driver.
#[derive(Debug, Clone, prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// Reasons for dropped solutions.
    #[metric(labels("solver", "reason"))]
    pub dropped_solutions: prometheus::IntCounterVec,
    /// The results of the solving process.
    #[metric(labels("solver", "result"))]
    pub solutions: prometheus::IntCounterVec,
    /// The results of the reveal process.
    #[metric(labels("solver", "result"))]
    pub reveals: prometheus::IntCounterVec,
    /// The results of the settlement process.
    #[metric(labels("solver", "result"))]
    pub settlements: prometheus::IntCounterVec,
    /// The results of the quoting process.
    #[metric(labels("solver", "result"))]
    pub quotes: prometheus::IntCounterVec,
    /// The results of the mempool submission.
    #[metric(labels("mempool", "result"))]
    pub mempool_submission: prometheus::IntCounterVec,
    /// How many tokens detected by specific solver and strategy.
    #[metric(labels("solver", "strategy"))]
    pub bad_tokens_detected: prometheus::IntCounterVec,
    /// Time spent in the auction preprocessing stage.
    #[metric(
        labels("stage"),
        buckets(
            0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0
        )
    )]
    pub auction_preprocessing: prometheus::HistogramVec,
}

/// Setup the metrics registry.
pub fn init() {
    observe::metrics::setup_registry_reentrant(Some("driver".to_owned()), None);
}

/// Get the metrics instance.
pub fn get() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
}
