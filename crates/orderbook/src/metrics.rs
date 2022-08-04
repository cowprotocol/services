use anyhow::Result;
use gas_estimation::GasPrice1559;
use prometheus::{Gauge, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts};
use shared::sources::{
    balancer_v2::pool_fetching::BalancerPoolCacheMetrics, uniswap_v2::pool_cache::PoolCacheMetrics,
};
use std::time::Duration;

pub struct Metrics {
    pool_cache_hits: IntCounter,
    pool_cache_misses: IntCounter,
    /// Gas estimate metrics
    gas_price: Gauge,
    price_estimates: IntCounterVec,
    native_price_cache: IntCounterVec,
    price_estimation_times: HistogramVec,
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

        let pool_cache_hits = IntCounter::new(
            "pool_cache_hits",
            "Number of cache hits in the pool fetcher cache.",
        )?;
        registry.register(Box::new(pool_cache_hits.clone()))?;
        let pool_cache_misses = IntCounter::new(
            "pool_cache_misses",
            "Number of cache misses in the pool fetcher cache.",
        )?;
        registry.register(Box::new(pool_cache_misses.clone()))?;

        let opts = Opts::new("gas_price", "Gas price estimate over time.");
        let gas_price = Gauge::with_opts(opts).unwrap();
        registry.register(Box::new(gas_price.clone()))?;

        let price_estimates = IntCounterVec::new(
            Opts::new("price_estimates", "Price estimator success/failure counter"),
            &["estimator_type", "result"],
        )?;
        registry.register(Box::new(price_estimates.clone()))?;

        let native_price_cache = IntCounterVec::new(
            Opts::new("native_price_cache", "Native price cache hit/miss counter."),
            &["result"],
        )?;
        registry.register(Box::new(native_price_cache.clone()))?;

        let price_estimation_times = HistogramVec::new(
            HistogramOpts::new("price_estimation_times", "Times for price estimations"),
            &["estimator_type", "time_spent_estimating"],
        )
        .unwrap();
        registry.register(Box::new(price_estimation_times.clone()))?;

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
            pool_cache_hits,
            pool_cache_misses,
            gas_price,
            price_estimates,
            native_price_cache,
            price_estimation_times,
            auction_creations,
            auction_solvable_orders,
            auction_filtered_orders,
            auction_errored_price_estimates,
            auction_price_estimate_timeouts,
        })
    }
}

impl PoolCacheMetrics for Metrics {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize) {
        self.pool_cache_hits.inc_by(cache_hits as u64);
        self.pool_cache_misses.inc_by(cache_misses as u64);
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

impl crate::gas_price::Metrics for Metrics {
    fn gas_price(&self, estimate: GasPrice1559) {
        self.gas_price.set(estimate.effective_gas_price() / 1e9);
    }
}

impl shared::price_estimation::instrumented::Metrics for Metrics {
    fn initialize_estimator(&self, name: &str) {
        for result in ["success", "failure"] {
            self.price_estimates
                .with_label_values(&[name, result])
                .reset();
        }
    }

    fn price_estimated(&self, name: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        self.price_estimates
            .with_label_values(&[name, result])
            .inc();
    }

    fn price_estimation_timed(&self, name: &str, time: Duration) {
        self.price_estimation_times
            .with_label_values(&[name, "time_spent_estimating"])
            .observe(time.as_secs_f64());
    }
}

impl BalancerPoolCacheMetrics for Metrics {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize) {
        // We may want to distinguish cache metrics between the different
        // liquidity sources in the future, for now just use the same counters.
        self.pool_cache_hits.inc_by(cache_hits as u64);
        self.pool_cache_misses.inc_by(cache_misses as u64);
    }
}

impl shared::price_estimation::native_price_cache::Metrics for Metrics {
    fn native_price_cache(&self, misses: usize, hits: usize) {
        self.native_price_cache
            .with_label_values(&["misses"])
            .inc_by(misses as u64);
        self.native_price_cache
            .with_label_values(&["hits"])
            .inc_by(hits as u64);
    }
}

pub struct NoopMetrics;

impl crate::solvable_orders::AuctionMetrics for NoopMetrics {
    fn auction_updated(&self, _: u64, _: u64, _: u64, _: bool) {}
}
