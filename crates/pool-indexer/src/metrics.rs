use {prometheus::HistogramVec, prometheus_metric_storage::MetricStorage};

#[derive(MetricStorage)]
#[metric(subsystem = "pool_indexer")]
pub struct Metrics {
    /// Per-chunk commit duration in seconds. The histogram's `_count` series
    /// doubles as the "chunks committed" rate.
    #[metric(
        labels("network"),
        buckets(0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0)
    )]
    pub chunk_commit_seconds: HistogramVec,

    /// Events applied to the DB, labelled by type.
    #[metric(labels("network", "kind"))]
    pub events_applied: prometheus::IntCounterVec,

    /// Highest block committed by the live indexer for this chain.
    #[metric(labels("network"))]
    pub indexed_block: prometheus::IntGaugeVec,

    /// Lag (in blocks) between the chain's finalized/latest tip and the
    /// indexer's checkpoint. Sampled each polling tick.
    #[metric(labels("network"))]
    pub indexer_lag_blocks: prometheus::IntGaugeVec,

    /// Unrecoverable `run_once` errors that forced a retry.
    #[metric(labels("network"))]
    pub indexer_errors: prometheus::IntCounterVec,

    /// Tokens still needing a value for the given `field` (`symbol` or
    /// `decimals`), sampled each backfill pass.
    #[metric(labels("network", "field"))]
    pub backfill_pending: prometheus::IntGaugeVec,

    /// Rows written by the backfill, labelled by `field` (`symbol` or
    /// `decimals`) and `result` (`ok` for a real value, `empty` for the
    /// "tried and failed" sentinel).
    #[metric(labels("network", "field", "result"))]
    pub backfilled: prometheus::IntCounterVec,

    /// API request count by route + HTTP status.
    #[metric(labels("route", "status"))]
    pub api_requests: prometheus::IntCounterVec,

    /// API request duration.
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

/// Extension trait that adds a `timer(&[labels])` method to [`HistogramVec`].
/// Returns a guard that records the elapsed time on drop.
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
