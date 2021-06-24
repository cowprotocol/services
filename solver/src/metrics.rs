use std::{
    convert::TryInto,
    time::{Duration, Instant},
};

use anyhow::Result;
use model::order::Order;
use prometheus::{
    HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGaugeVec, Opts, Registry,
};
use shared::{pool_cache::PoolCacheMetrics, transport::instrumented::TransportMetrics};
use strum::{AsStaticRef, VariantNames};

use crate::liquidity::Liquidity;

pub trait SolverMetrics {
    fn liquidity_fetched(&self, liquidity: &[Liquidity]);
    fn settlement_computed(&self, solver_type: &str, start: Instant);
    fn order_settled(&self, order: &Order, solver: &'static str);
    fn settlement_simulation_succeeded(&self, solver: &'static str);
    fn settlement_simulation_failed(&self, solver: &'static str);
    fn settlement_submitted(&self, successful: bool, solver: &'static str);
    fn orders_matched_but_not_settled(&self, count: usize);
}

// TODO add labeled interaction counter once we support more than one interaction
pub struct Metrics {
    trade_counter: IntCounterVec,
    order_settlement_time: IntCounter,
    solver_computation_time: IntCounterVec,
    liquidity: IntGaugeVec,
    settlement_simulations: IntCounterVec,
    settlement_submissions: IntCounterVec,
    matched_but_unsettled_orders: IntCounter,
    transport_requests: HistogramVec,
    pool_cache_hits: IntCounter,
    pool_cache_misses: IntCounter,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Result<Self> {
        let trade_counter = IntCounterVec::new(
            Opts::new("gp_v2_solver_trade_counter", "Number of trades settled"),
            &["solver_type"],
        )?;
        registry.register(Box::new(trade_counter.clone()))?;

        let order_settlement_time = IntCounter::new(
            "gp_v2_solver_order_settlement_time_seconds",
            "Counter for the number of seconds between creation and settlement of an order",
        )?;
        registry.register(Box::new(order_settlement_time.clone()))?;

        let solver_computation_time = IntCounterVec::new(
            Opts::new(
                "gp_v2_solver_computation_time_ms",
                "Ms each solver takes to compute their solution",
            ),
            &["solver_type"],
        )?;
        registry.register(Box::new(solver_computation_time.clone()))?;

        let liquidity = IntGaugeVec::new(
            Opts::new(
                "gp_v2_solver_liquidity_gauge",
                "Amount of orders labeled by liquidity type currently available to the solvers",
            ),
            &["liquidity_type"],
        )?;
        registry.register(Box::new(liquidity.clone()))?;

        let settlement_simulations = IntCounterVec::new(
            Opts::new(
                "gp_v2_solver_settlement_simulations",
                "Settlement simulation counts",
            ),
            &["result", "solver_type"],
        )?;
        registry.register(Box::new(settlement_simulations.clone()))?;

        let settlement_submissions = IntCounterVec::new(
            Opts::new(
                "gp_v2_solver_settlement_submissions",
                "Settlement submission counts",
            ),
            &["result", "solver_type"],
        )?;
        registry.register(Box::new(settlement_submissions.clone()))?;

        let matched_but_unsettled_orders = IntCounter::new(
            "gp_v2_solver_orders_matched_not_settled",
            "Counter for the number of orders for which at least one solver computed an execution which was not chosen in this run-loop",
        )?;
        registry.register(Box::new(matched_but_unsettled_orders.clone()))?;

        let opts = HistogramOpts::new(
            "gp_v2_solver_transport_requests",
            "RPC Request durations labelled by method",
        );
        let transport_requests = HistogramVec::new(opts, &["method"]).unwrap();
        registry.register(Box::new(transport_requests.clone()))?;

        let pool_cache_hits = IntCounter::new(
            "gp_v2_solver_pool_cache_hits",
            "Number of cache hits in the pool fetcher cache.",
        )?;
        registry.register(Box::new(pool_cache_hits.clone()))?;

        let pool_cache_misses = IntCounter::new(
            "gp_v2_solver_pool_cache_misses",
            "Number of cache misses in the pool fetcher cache.",
        )?;
        registry.register(Box::new(pool_cache_misses.clone()))?;

        Ok(Self {
            trade_counter,
            order_settlement_time,
            solver_computation_time,
            liquidity,
            settlement_simulations,
            settlement_submissions,
            matched_but_unsettled_orders,
            transport_requests,
            pool_cache_hits,
            pool_cache_misses,
        })
    }
}

impl SolverMetrics for Metrics {
    fn liquidity_fetched(&self, liquidity: &[Liquidity]) {
        // Reset all gauges and start from scratch
        Liquidity::VARIANTS.iter().for_each(|label| {
            self.liquidity.with_label_values(&[label]).set(0);
        });
        liquidity.iter().for_each(|liquidity| {
            let label = liquidity.as_static();
            self.liquidity.with_label_values(&[label]).inc();
        })
    }

    fn settlement_computed(&self, solver_type: &str, start: Instant) {
        self.solver_computation_time
            .with_label_values(&[solver_type])
            .inc_by(
                Instant::now()
                    .duration_since(start)
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            )
    }

    fn order_settled(&self, order: &Order, solver: &'static str) {
        let time_to_settlement =
            chrono::offset::Utc::now().signed_duration_since(order.order_meta_data.creation_date);
        self.trade_counter.with_label_values(&[solver]).inc();
        self.order_settlement_time.inc_by(
            time_to_settlement
                .num_seconds()
                .try_into()
                .unwrap_or_default(),
        )
    }

    fn settlement_simulation_succeeded(&self, solver: &'static str) {
        self.settlement_simulations
            .with_label_values(&["success", solver])
            .inc()
    }

    fn settlement_simulation_failed(&self, solver: &'static str) {
        self.settlement_simulations
            .with_label_values(&["failure", solver])
            .inc()
    }

    fn settlement_submitted(&self, successful: bool, solver: &'static str) {
        let result = if successful { "success" } else { "failures" };
        self.settlement_submissions
            .with_label_values(&[result, solver])
            .inc()
    }

    fn orders_matched_but_not_settled(&self, count: usize) {
        self.matched_but_unsettled_orders.inc_by(count as u64);
    }
}

impl TransportMetrics for Metrics {
    fn report_query(&self, label: &str, elapsed: Duration) {
        self.transport_requests
            .with_label_values(&[label])
            .observe(elapsed.as_secs_f64())
    }
}

impl PoolCacheMetrics for Metrics {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize) {
        self.pool_cache_hits.inc_by(cache_hits as u64);
        self.pool_cache_misses.inc_by(cache_misses as u64);
    }
}

#[derive(Default)]
pub struct NoopMetrics {}

impl SolverMetrics for NoopMetrics {
    fn liquidity_fetched(&self, _liquidity: &[Liquidity]) {}
    fn settlement_computed(&self, _solver_type: &str, _start: Instant) {}
    fn order_settled(&self, _: &Order, _: &'static str) {}
    fn settlement_simulation_succeeded(&self, _: &'static str) {}
    fn settlement_simulation_failed(&self, _: &'static str) {}
    fn settlement_submitted(&self, _: bool, _: &'static str) {}
    fn orders_matched_but_not_settled(&self, _: usize) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_work() {
        let registry = Registry::default();
        let metrics = Metrics::new(&registry).unwrap();
        metrics.settlement_computed("asdf", Instant::now());
        metrics.order_settled(&Default::default(), "test");
        metrics.settlement_simulation_succeeded("test");
        metrics.settlement_simulation_failed("test");
        metrics.settlement_submitted(true, "test");
        metrics.orders_matched_but_not_settled(20);
    }
}
