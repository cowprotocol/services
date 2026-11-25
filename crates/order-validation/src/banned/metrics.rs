//! Instrumentation of the Hermod batch lookups and of the cache in front of
//! them.

use {
    prometheus::{HistogramVec, IntCounterVec},
    std::time::Duration,
};

const BACKEND: &str = "hermod";

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "banned_users")]
pub(super) struct Metrics {
    /// Checked addresses by outcome: banned, not_banned or error (the batch
    /// request failed; only the local cache was used).
    #[metric(labels("backend", "result"))]
    lookups: IntCounterVec,

    /// Wall-clock time of a single batch request, successful or not.
    #[metric(
        labels("backend"),
        buckets(0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0)
    )]
    lookup_seconds: HistogramVec,

    /// Address checks by cache result: hit, or miss (forwarded to Hermod).
    #[metric(labels("result"))]
    cache: IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    pub(super) fn cache(hits: usize, misses: usize) {
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

    /// Records one batch request of `size` addresses. `hits` is `None` when
    /// the request failed.
    pub(super) fn batch(size: usize, hits: Option<usize>, elapsed: Duration) {
        let metrics = Self::get();
        let count = |result, n: usize| {
            metrics
                .lookups
                .with_label_values(&[BACKEND, result])
                .inc_by(n as u64);
        };
        match hits {
            Some(hits) => {
                count("banned", hits);
                count("not_banned", size.saturating_sub(hits));
            }
            None => count("error", size),
        }
        metrics
            .lookup_seconds
            .with_label_values(&[BACKEND])
            .observe(elapsed.as_secs_f64());
    }
}
