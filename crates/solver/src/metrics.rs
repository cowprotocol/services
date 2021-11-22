use crate::liquidity::{LimitOrder, Liquidity};
use anyhow::Result;
use ethcontract::U256;
use model::order::Order;
use prometheus::{
    Gauge, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGaugeVec, Opts,
};
use shared::{
    metrics::get_metrics_registry,
    metrics::LivenessChecking,
    sources::{
        balancer_v2::pool_cache::BalancerPoolCacheMetrics, uniswap_v2::pool_cache::PoolCacheMetrics,
    },
    transport::instrumented::TransportMetrics,
};
use std::{
    convert::TryInto,
    sync::Mutex,
    time::{Duration, Instant},
};
use strum::VariantNames;

/// The maximum time between the completion of two run loops. If exceeded the service will be considered unhealthy.
const MAX_RUNLOOP_DURATION: Duration = Duration::from_secs(7 * 60);

pub trait SolverMetrics: Send + Sync {
    fn orders_fetched(&self, orders: &[LimitOrder]);
    fn liquidity_fetched(&self, liquidity: &[Liquidity]);
    fn settlement_computed(&self, solver_type: &str, start: Instant);
    fn order_settled(&self, order: &Order, solver: &'static str);
    fn settlement_simulation_succeeded(&self, solver: &'static str);
    fn settlement_simulation_failed_on_latest(&self, solver: &'static str);
    fn solver_run_succeeded(&self, solver: &'static str);
    fn solver_run_failed(&self, solver: &'static str);
    fn single_order_solver_succeeded(&self, solver: &'static str);
    fn single_order_solver_failed(&self, solver: &'static str);
    fn settlement_simulation_failed(&self, solver: &'static str);
    fn settlement_submitted(&self, successful: bool, solver: &'static str);
    fn orders_matched_but_not_settled(&self, count: usize);
    fn report_order_surplus(&self, surplus_diff: f64);
    fn runloop_completed(&self);
    fn complete_runloop_until_transaction(&self, duration: Duration);
    fn transaction_submission(&self, duration: Duration);
    fn transaction_gas_price(&self, gas_price: U256);
}

// TODO add labeled interaction counter once we support more than one interaction
pub struct Metrics {
    trade_counter: IntCounterVec,
    order_settlement_time: IntCounter,
    solver_computation_time: IntCounterVec,
    liquidity: IntGaugeVec,
    settlement_simulations: IntCounterVec,
    settlement_submissions: IntCounterVec,
    solver_runs: IntCounterVec,
    single_order_solver_runs: IntCounterVec,
    matched_but_unsettled_orders: IntCounter,
    transport_requests: HistogramVec,
    pool_cache_hits: IntCounter,
    pool_cache_misses: IntCounter,
    last_runloop_completed: Mutex<Instant>,
    order_surplus_report: Histogram,
    complete_runloop_until_transaction: Histogram,
    transaction_submission: Histogram,
    transaction_gas_price_gwei: Gauge,
}

impl Metrics {
    pub fn new() -> Result<Self> {
        let registry = get_metrics_registry();

        let trade_counter = IntCounterVec::new(
            Opts::new("trade_counter", "Number of trades settled"),
            &["solver_type"],
        )?;
        registry.register(Box::new(trade_counter.clone()))?;

        let order_settlement_time = IntCounter::new(
            "order_settlement_time_seconds",
            "Counter for the number of seconds between creation and settlement of an order",
        )?;
        registry.register(Box::new(order_settlement_time.clone()))?;

        let solver_computation_time = IntCounterVec::new(
            Opts::new(
                "computation_time_ms",
                "Ms each solver takes to compute their solution",
            ),
            &["solver_type"],
        )?;
        registry.register(Box::new(solver_computation_time.clone()))?;

        let liquidity = IntGaugeVec::new(
            Opts::new(
                "liquidity_gauge",
                "Amount of orders labeled by liquidity type currently available to the solvers",
            ),
            &["liquidity_type"],
        )?;
        registry.register(Box::new(liquidity.clone()))?;

        let settlement_simulations = IntCounterVec::new(
            Opts::new("settlement_simulations", "Settlement simulation counts"),
            &["result", "solver_type"],
        )?;
        registry.register(Box::new(settlement_simulations.clone()))?;

        let settlement_submissions = IntCounterVec::new(
            Opts::new("settlement_submissions", "Settlement submission counts"),
            &["result", "solver_type"],
        )?;
        registry.register(Box::new(settlement_submissions.clone()))?;

        let solver_runs = IntCounterVec::new(
            Opts::new("solver_run", "Success/Failure counts"),
            &["result", "solver_type"],
        )?;
        registry.register(Box::new(solver_runs.clone()))?;

        let single_order_solver_runs = IntCounterVec::new(
            Opts::new("single_order_solver", "Success/Failure counts"),
            &["result", "solver_type"],
        )?;
        registry.register(Box::new(single_order_solver_runs.clone()))?;

        let matched_but_unsettled_orders = IntCounter::new(
            "orders_matched_not_settled",
            "Counter for the number of orders for which at least one solver computed an execution which was not chosen in this run-loop",
        )?;
        registry.register(Box::new(matched_but_unsettled_orders.clone()))?;

        let order_surplus_report = Histogram::with_opts(
            HistogramOpts::new(
                "settlement_surplus_report",
                "Surplus ratio differences between winning and best settlement per order",
            )
            .buckets(vec![-1.0, -0.1, -0.01, -0.005, 0., 0.005, 0.01, 0.1, 1.0]),
        )?;
        registry.register(Box::new(order_surplus_report.clone()))?;

        let opts = HistogramOpts::new(
            "transport_requests",
            "RPC Request durations labelled by method",
        );
        let transport_requests = HistogramVec::new(opts, &["method"]).unwrap();
        registry.register(Box::new(transport_requests.clone()))?;

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

        let opts = prometheus::opts!(
            "complete_runloop_until_transaction_seconds",
            "Time a runloop that wants to submit a solution takes until the transaction submission starts."
        );
        let complete_runloop_until_transaction = Histogram::with_opts(HistogramOpts {
            common_opts: opts,
            buckets: vec![f64::INFINITY],
        })?;
        registry.register(Box::new(complete_runloop_until_transaction.clone()))?;

        let opts = prometheus::opts!(
            "transaction_submission_seconds",
            "Time it takes to submit a settlement transaction."
        );
        let transaction_submission = Histogram::with_opts(HistogramOpts {
            common_opts: opts,
            buckets: vec![f64::INFINITY],
        })?;
        registry.register(Box::new(transaction_submission.clone()))?;

        let opts = Opts::new(
            "transaction_gas_price_gwei",
            "Actual gas price used by settlement transaction.",
        );
        let transaction_gas_price_gwei = Gauge::with_opts(opts).unwrap();
        registry.register(Box::new(transaction_gas_price_gwei.clone()))?;

        Ok(Self {
            trade_counter,
            order_settlement_time,
            solver_computation_time,
            liquidity,
            settlement_simulations,
            settlement_submissions,
            solver_runs,
            single_order_solver_runs,
            matched_but_unsettled_orders,
            transport_requests,
            pool_cache_hits,
            pool_cache_misses,
            last_runloop_completed: Mutex::new(Instant::now()),
            order_surplus_report,
            complete_runloop_until_transaction,
            transaction_submission,
            transaction_gas_price_gwei,
        })
    }
}

impl SolverMetrics for Metrics {
    fn orders_fetched(&self, orders: &[LimitOrder]) {
        self.liquidity
            .with_label_values(&["Limit"])
            .set(orders.len() as _);
    }

    fn liquidity_fetched(&self, liquidity: &[Liquidity]) {
        // Reset all gauges and start from scratch
        Liquidity::VARIANTS.iter().for_each(|label| {
            self.liquidity.with_label_values(&[label]).set(0);
        });
        liquidity.iter().for_each(|liquidity| {
            let label: &'static str = liquidity.into();
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

    fn settlement_simulation_failed_on_latest(&self, solver: &'static str) {
        self.settlement_simulations
            .with_label_values(&["failure_on_latest", solver])
            .inc()
    }

    fn solver_run_succeeded(&self, solver: &'static str) {
        self.solver_runs
            .with_label_values(&["success", solver])
            .inc()
    }

    fn solver_run_failed(&self, solver: &'static str) {
        self.solver_runs
            .with_label_values(&["failure", solver])
            .inc()
    }

    fn single_order_solver_succeeded(&self, solver: &'static str) {
        self.single_order_solver_runs
            .with_label_values(&["success", solver])
            .inc()
    }

    fn single_order_solver_failed(&self, solver: &'static str) {
        self.single_order_solver_runs
            .with_label_values(&["failure", solver])
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

    fn report_order_surplus(&self, surplus_diff: f64) {
        self.order_surplus_report.observe(surplus_diff)
    }

    fn runloop_completed(&self) {
        *self
            .last_runloop_completed
            .lock()
            .expect("thread holding mutex panicked") = Instant::now();
    }

    fn complete_runloop_until_transaction(&self, duration: Duration) {
        self.complete_runloop_until_transaction
            .observe(duration.as_secs_f64());
    }

    fn transaction_submission(&self, duration: Duration) {
        self.transaction_submission.observe(duration.as_secs_f64());
    }

    fn transaction_gas_price(&self, gas_price: U256) {
        self.transaction_gas_price_gwei
            .set(gas_price.to_f64_lossy() / 1e9)
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

impl BalancerPoolCacheMetrics for Metrics {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize) {
        // We may want to distinguish cache metrics between the different
        // liquidity sources in the future, for now just use the same counters.
        self.pool_cache_hits.inc_by(cache_hits as u64);
        self.pool_cache_misses.inc_by(cache_misses as u64);
    }
}

#[async_trait::async_trait]
impl LivenessChecking for Metrics {
    async fn is_alive(&self) -> bool {
        Instant::now().duration_since(
            *self
                .last_runloop_completed
                .lock()
                .expect("thread holding mutex panicked"),
        ) <= MAX_RUNLOOP_DURATION
    }
}

#[derive(Default)]
pub struct NoopMetrics {}

impl SolverMetrics for NoopMetrics {
    fn orders_fetched(&self, _liquidity: &[LimitOrder]) {}
    fn liquidity_fetched(&self, _liquidity: &[Liquidity]) {}
    fn settlement_computed(&self, _solver_type: &str, _start: Instant) {}
    fn order_settled(&self, _: &Order, _: &'static str) {}
    fn settlement_simulation_succeeded(&self, _: &'static str) {}
    fn settlement_simulation_failed_on_latest(&self, _: &'static str) {}
    fn solver_run_succeeded(&self, _: &'static str) {}
    fn solver_run_failed(&self, _: &'static str) {}
    fn single_order_solver_succeeded(&self, _: &'static str) {}
    fn single_order_solver_failed(&self, _: &'static str) {}
    fn settlement_simulation_failed(&self, _: &'static str) {}
    fn settlement_submitted(&self, _: bool, _: &'static str) {}
    fn orders_matched_but_not_settled(&self, _: usize) {}
    fn report_order_surplus(&self, _: f64) {}
    fn runloop_completed(&self) {}
    fn complete_runloop_until_transaction(&self, _: Duration) {}
    fn transaction_submission(&self, _: Duration) {}
    fn transaction_gas_price(&self, _: U256) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_work() {
        let metrics = Metrics::new().unwrap();
        metrics.settlement_computed("asdf", Instant::now());
        metrics.order_settled(&Default::default(), "test");
        metrics.settlement_simulation_succeeded("test");
        metrics.settlement_simulation_failed("test");
        metrics.settlement_submitted(true, "test");
        metrics.orders_matched_but_not_settled(20);
    }
}
