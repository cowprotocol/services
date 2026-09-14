//! Instrumentation shared by every banned-user backend and by the cache in
//! front of them.

use {
    super::Source,
    prometheus::{HistogramVec, IntCounterVec, IntGaugeVec},
    std::time::Duration,
};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "banned_users")]
pub(super) struct Metrics {
    /// Backend lookups by outcome: banned, not_banned or error (lookup failed
    /// and was ignored). Includes background refreshes, so a long-lived banned
    /// address is counted repeatedly - see `detected` for the deduplicated
    /// count.
    #[metric(labels("backend", "result"))]
    lookups: IntCounterVec,

    /// Wall-clock time of a single backend lookup, successful or not.
    #[metric(
        labels("backend"),
        buckets(0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0)
    )]
    lookup_seconds: HistogramVec,

    /// Address checks by cache result: hit, or miss (forwarded to the
    /// backends).
    #[metric(labels("result"))]
    cache: IntCounterVec,

    /// Newly banned addresses, by the source that reported them. Counted once
    /// per address and source for as long as the address stays cached, so
    /// repeated checks and background refreshes of an already known address do
    /// not inflate it.
    #[metric(labels("source"))]
    detected: IntCounterVec,

    /// Banned addresses currently held in the cache, by the source that
    /// reported them. An address several sources report counts once under
    /// each, so the series do not sum to the number of banned addresses.
    #[metric(labels("source"))]
    currently_banned: IntGaugeVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    pub(super) fn lookup(backend: Source, result: Result<bool, ()>, elapsed: Duration) {
        let result = match result {
            Ok(true) => "banned",
            Ok(false) => "not_banned",
            Err(()) => "error",
        };
        let metrics = Self::get();
        metrics
            .lookups
            .with_label_values(&[backend.as_str(), result])
            .inc();
        metrics
            .lookup_seconds
            .with_label_values(&[backend.as_str()])
            .observe(elapsed.as_secs_f64());
    }

    pub(super) fn cache_hits(hits: usize, misses: usize) {
        let metrics = Self::get();
        metrics
            .cache
            .with_label_values(&["hit"])
            .inc_by(hits as u64);
        metrics
            .cache
            .with_label_values(&["miss"])
            .inc_by(misses as u64);
    }

    pub(super) fn detected(source: Source) {
        Self::get()
            .detected
            .with_label_values(&[source.as_str()])
            .inc();
    }

    pub(super) fn currently_banned(source: Source, count: i64) {
        Self::get()
            .currently_banned
            .with_label_values(&[source.as_str()])
            .set(count);
    }
}
