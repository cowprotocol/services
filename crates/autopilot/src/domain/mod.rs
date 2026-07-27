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
}
