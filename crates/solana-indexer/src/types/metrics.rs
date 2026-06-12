//! Prometheus metric handles.

use prometheus::{
    IntCounter,
    IntCounterVec,
    core::{AtomicI64, GenericGauge},
};

/// Prometheus metrics for the solana-indexer.
#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "solana_indexer")]
pub struct Metrics {
    /// Slots between the chain tip and the indexed watermark.
    pub indexer_lag_slots: GenericGauge<AtomicI64>,

    /// Slots between the chain tip and the oldest confirmed-but-not-finalized
    /// row.
    pub commitment_promotion_lag: GenericGauge<AtomicI64>,

    /// Dead-letter events, broken down by reason.
    #[metric(labels("reason"))]
    pub partial_event_dead_letter_total: IntCounterVec,

    /// Decode failures diverted to `solana.dead_letter`.
    pub decode_errors_total: IntCounter,
}

/// Returns the global [`Metrics`] singleton, initialising it on first call.
pub fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}
