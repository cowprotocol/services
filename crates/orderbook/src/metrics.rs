use anyhow::Result;
use prometheus::{IntCounter, IntGauge};

pub struct Metrics {
    // auction metrics
    auction_creations: IntCounter,
    auction_solvable_orders: IntGauge,
    auction_filtered_orders: IntGauge,
    auction_errored_price_estimates: IntCounter,
    auction_price_estimate_timeouts: IntCounter,
}

impl Metrics {
    pub fn new() -> Result<Self> {
        let registry = global_metrics::get_metrics_registry();

        let auction_creations = IntCounter::new(
            "auction_creations",
            "Number of times an auction has been created.",
        )?;
        registry.register(Box::new(auction_creations.clone()))?;

        let auction_solvable_orders = IntGauge::new(
            "auction_solvable_orders",
            "Number of orders that are in the current auction.",
        )?;
        registry.register(Box::new(auction_solvable_orders.clone()))?;

        let auction_filtered_orders = IntGauge::new(
            "auction_filtered_orders",
            "Number of orders that have been filtered out in the current auction.",
        )?;
        registry.register(Box::new(auction_filtered_orders.clone()))?;

        let auction_errored_price_estimates = IntCounter::new(
            "auction_errored_price_estimates",
            "Number of native price estimates that errored when creating auction.",
        )?;
        registry.register(Box::new(auction_errored_price_estimates.clone()))?;

        let auction_price_estimate_timeouts = IntCounter::new(
            "auction_price_estimate_timeouts",
            "Number of times auction creation didn't get all native price estimates in time.",
        )?;
        registry.register(Box::new(auction_price_estimate_timeouts.clone()))?;

        Ok(Self {
            auction_creations,
            auction_solvable_orders,
            auction_filtered_orders,
            auction_errored_price_estimates,
            auction_price_estimate_timeouts,
        })
    }
}

impl crate::solvable_orders::AuctionMetrics for Metrics {
    fn auction_updated(
        &self,
        solvable_orders: u64,
        filtered_orders: u64,
        errored_estimates: u64,
        timeout: bool,
    ) {
        self.auction_creations.inc();
        self.auction_solvable_orders.set(solvable_orders as i64);
        if timeout {
            self.auction_price_estimate_timeouts.inc();
        }
        self.auction_filtered_orders.set(filtered_orders as i64);
        self.auction_errored_price_estimates
            .inc_by(errored_estimates);
    }
}

pub struct NoopMetrics;

impl crate::solvable_orders::AuctionMetrics for NoopMetrics {
    fn auction_updated(&self, _: u64, _: u64, _: u64, _: bool) {}
}
