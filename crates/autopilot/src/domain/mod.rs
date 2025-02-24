pub mod auction;
pub mod competition;
pub mod eth;
pub mod fee;
pub mod quote;
pub mod settlement;

pub use {
    auction::{
        order::{Order, OrderUid},
        Auction,
        RawAuctionData,
    },
    fee::ProtocolFees,
    quote::Quote,
};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "domain")]
pub struct Metrics {
    /// How many times the solver was marked as non-settling based on the
    /// database statistics.
    #[metric(labels("solver"))]
    pub non_settling_solver: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}
