//! Instrumentation shared by every banned-user backend and by the cache in
//! front of them.

use {
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

    /// Newly banned addresses, by the backend that reported them. Counted
    /// once per address and backend for as long as the address stays cached,
    /// so repeated checks and refreshes of a known address do not inflate it.
    #[metric(labels("backend"))]
    detected: IntCounterVec,

    /// Banned addresses currently held in the cache, by the backend that
    /// reported them. An address several backends report counts under each.
    #[metric(labels("backend"))]
    currently_banned: IntGaugeVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    pub(super) fn lookup(backend: &str, result: Result<bool, ()>, elapsed: Duration) {
        let result = match result {
            Ok(true) => "banned",
            Ok(false) => "not_banned",
            Err(()) => "error",
        };
        let metrics = Self::get();
        metrics.lookups.with_label_values(&[backend, result]).inc();
        metrics
            .lookup_seconds
            .with_label_values(&[backend])
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

    pub(super) fn detected(backend: &str) {
        Self::get().detected.with_label_values(&[backend]).inc();
    }

    pub(super) fn currently_banned(backend: &str, count: usize) {
        Self::get()
            .currently_banned
            .with_label_values(&[backend])
            .set(i64::try_from(count).unwrap_or(i64::MAX));
    }
}
