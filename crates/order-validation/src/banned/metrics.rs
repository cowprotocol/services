//! Instrumentation shared by every banned-user backend and by the cache in
//! front of them.

use {
    prometheus::{HistogramVec, IntCounter, IntCounterVec, IntGauge},
    std::time::Duration,
};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "banned_users")]
pub(super) struct Metrics {
    /// Backend lookups by outcome. `banned` and `not_banned` are the answers
    /// a backend gave; `error` means the lookup failed and was ignored.
    /// Background refreshes are included, so a long-lived banned address is
    /// counted repeatedly here — see `detected` for the deduplicated count.
    #[metric(labels("backend", "result"))]
    lookups: IntCounterVec,

    /// Wall-clock time of a single backend lookup, successful or not.
    #[metric(
        labels("backend"),
        buckets(0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0)
    )]
    lookup_seconds: HistogramVec,

    /// Address checks served from the cache (`hit`) versus forwarded to the
    /// backends (`miss`).
    #[metric(labels("result"))]
    cache: IntCounterVec,

    /// Addresses whose verdict turned banned. Counted once per address for as
    /// long as it stays cached, so repeated checks and background refreshes of
    /// an already known address do not inflate it.
    detected: IntCounter,

    /// Addresses currently held in the cache with a banned verdict.
    currently_banned: IntGauge,
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

    pub(super) fn detected() {
        Self::get().detected.inc();
    }

    pub(super) fn currently_banned(count: usize) {
        Self::get()
            .currently_banned
            .set(i64::try_from(count).unwrap_or(i64::MAX));
    }
}
