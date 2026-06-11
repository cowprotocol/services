use {prometheus::HistogramVec, prometheus_metric_storage::MetricStorage};

#[derive(MetricStorage)]
#[metric(subsystem = "pool_indexer")]
pub struct Metrics {
    /// Chunk commit duration. The `_count` series doubles as a chunks-
    /// committed rate.
    #[metric(
        labels("network"),
        buckets(0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0)
    )]
    pub chunk_commit_seconds: HistogramVec,

    /// Events applied to the DB, labelled by event type.
    #[metric(labels("network", "kind"))]
    pub events_applied: prometheus::IntCounterVec,

    /// Highest block committed by the live indexer.
    #[metric(labels("network"))]
    pub indexed_block: prometheus::IntGaugeVec,

    /// Blocks between the chain head and the indexer's checkpoint.
    /// Refreshed at the start of every `run_once` and after each chunk
    /// commit so dashboards can watch the lag drain in real time.
    #[metric(labels("network"))]
    pub indexer_lag_blocks: prometheus::IntGaugeVec,

    /// `run_once` failures that forced a retry.
    #[metric(labels("network"))]
    pub indexer_errors: prometheus::IntCounterVec,

    /// Tokens still missing `symbol` / `decimals`. Sampled each backfill pass.
    #[metric(labels("network", "field"))]
    pub backfill_pending: prometheus::IntGaugeVec,

    /// Backfill writes. `result=ok` is a real value, `result=empty` is the
    /// "tried and failed" sentinel (so we don't retry forever).
    #[metric(labels("network", "field", "result"))]
    pub backfilled: prometheus::IntCounterVec,

    /// API request count by route + HTTP status.
    #[metric(labels("route", "status"))]
    pub api_requests: prometheus::IntCounterVec,

    /// API request latency by route.
    #[metric(
        labels("route"),
        buckets(0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5)
    )]
    pub api_request_seconds: HistogramVec,
}

impl Metrics {
    pub fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry())
            .expect("unexpected pool_indexer metrics duplicate registration")
    }
}

/// `timer(&[labels])` on a [`HistogramVec`] returns a guard that records
/// elapsed wall time on drop.
pub trait HistogramVecExt {
    #[must_use]
    fn timer<'a>(&'a self, labels: &'a [&'a str]) -> impl Drop + use<'a, Self>;
}

impl HistogramVecExt for HistogramVec {
    fn timer<'a>(&'a self, labels: &'a [&'a str]) -> impl Drop + use<'a> {
        let start = std::time::Instant::now();
        scopeguard::guard(start, move |start| {
            self.with_label_values(labels)
                .observe(start.elapsed().as_secs_f64());
        })
    }
}
