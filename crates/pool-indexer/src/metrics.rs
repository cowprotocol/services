//! Prometheus metrics for pool-indexer.
//!
//! All metrics live under the `pool_indexer_` prefix (configured by
//! `observe::metrics::setup_registry`) and are labelled by `network` where
//! more than one network is active in the same process. Call `Metrics::get()`
//! to reach the shared registry-backed instance.

use {prometheus::HistogramVec, prometheus_metric_storage::MetricStorage};

#[derive(MetricStorage)]
#[metric(subsystem = "pool_indexer")]
pub struct Metrics {
    /// Chunks successfully committed to the DB.
    #[metric(labels("network"))]
    pub chunks_committed: prometheus::IntCounterVec,

    /// Per-chunk commit duration in seconds.
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

    /// Duration of each phase of the cold-seed bootstrap.
    #[metric(
        labels("network", "phase"),
        buckets(
            1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1_200.0, 1_800.0, 3_600.0
        )
    )]
    pub cold_seed_phase_seconds: HistogramVec,

    /// Pools discovered by the cold seeder (phase 1).
    #[metric(labels("network"))]
    pub cold_seed_pools_discovered: prometheus::IntGaugeVec,

    /// Duration of the full subgraph seed (pool page fetch + tick fetch).
    #[metric(
        labels("network"),
        buckets(1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0)
    )]
    pub subgraph_seed_seconds: HistogramVec,

    /// Symbols written to the DB (label: `result` = `ok` for a real symbol,
    /// `empty` for the "tried and failed" sentinel).
    #[metric(labels("network", "result"))]
    pub symbols_backfilled: prometheus::IntCounterVec,

    /// Tokens still needing a symbol, sampled each backfill pass.
    #[metric(labels("network"))]
    pub symbols_pending: prometheus::IntGaugeVec,

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

    /// Returns a guard that records the elapsed time on a histogram when it's
    /// dropped. Use with `let _timer = Metrics::timer(&hist, &[..]);` at the
    /// top of a function / block. Cleaner than manual `Instant::now()` +
    /// observe pairs, and records even on early return.
    #[must_use]
    pub fn timer<'a>(hist: &'a HistogramVec, labels: &'a [&'a str]) -> impl Drop + use<'a> {
        let start = std::time::Instant::now();
        scopeguard::guard(start, move |start| {
            hist.with_label_values(labels)
                .observe(start.elapsed().as_secs_f64());
        })
    }
}
