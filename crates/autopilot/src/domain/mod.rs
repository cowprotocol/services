pub mod auction;
pub mod blockchain;
pub mod competition;
pub mod fee;
pub mod quote;
pub mod settlement;

pub use {
    auction::{
        Auction,
        RawAuctionData,
        order::{Order, OrderUid},
    },
    fee::ProtocolFees,
    quote::Quote,
};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "domain")]
pub struct Metrics {
    /// Tracks settlements that couldn't be matched to the database solutions.
    #[metric(labels("solver_address"))]
    pub inconsistent_settlements: prometheus::IntCounterVec,

    /// Tracks trades whose surplus, fee or fee breakdown calculation failed
    /// and fell back to zeroed values.
    #[metric(labels("kind"))]
    pub settlement_math_errors: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    /// Publishes every `settlement_math_errors` series at zero. A counter that
    /// only appears with the first failure is born reading one, leaving
    /// `increase()` nothing to subtract from, so the alert would miss that
    /// first failure and only catch the second one.
    pub(crate) fn init_settlement_math_errors() {
        for kind in ["surplus", "fee", "fee_breakdown"] {
            Self::get()
                .settlement_math_errors
                .with_label_values(&[kind])
                .reset();
        }
    }
}
