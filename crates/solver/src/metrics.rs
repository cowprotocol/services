use {
    crate::{
        liquidity::{LimitOrder, Liquidity},
        settlement::Revertable,
    },
    anyhow::Result,
    ethcontract::U256,
    model::order::{Order, OrderClass},
    prometheus::{Gauge, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGaugeVec},
    shared::metrics::LivenessChecking,
    std::{
        convert::TryInto,
        sync::Mutex,
        time::{Duration, Instant},
    },
    strum::{IntoEnumIterator, VariantNames},
};

/// The maximum time between the completion of two run loops. If exceeded the
/// service will be considered unhealthy.
const MAX_RUNLOOP_DURATION: Duration = Duration::from_secs(7 * 60);

/// The outcome of a solver run.
#[derive(strum::EnumIter)]
pub enum SolverRunOutcome {
    /// Computed a non-trivial settlement.
    Success,
    /// Run succeeded (i.e. did not error), but solver produced no settlement or
    /// only trivial settlements.
    Empty,
    /// The solver timed out.
    Timeout,
    /// The solver returned an error.
    Failure,
}

impl SolverRunOutcome {
    fn label(&self) -> &'static str {
        match self {
            SolverRunOutcome::Success => "success",
            SolverRunOutcome::Empty => "empty",
            SolverRunOutcome::Timeout => "timeout",
            SolverRunOutcome::Failure => "failure",
        }
    }
}

/// The outcome of settlement submission.
#[derive(strum::EnumIter)]
pub enum SettlementSubmissionOutcome {
    /// A settlement transaction was mined and included on the blockchain.
    Success,
    /// A settlement transaction was mined and included on the blockchain but
    /// reverted.
    Revert,
    /// A transaction reverted in the simulation stage.
    SimulationRevert,
    /// Submission timed-out while waiting for the transaction to get mined.
    Timeout,
    /// Transaction sucessfully cancelled after simulation revert or timeout
    Cancel,
    /// Submission disabled
    Disabled,
    /// General message for failures (for example, failing to connect to client
    /// node)
    Failed,
}

impl SettlementSubmissionOutcome {
    fn label(&self) -> &'static str {
        match self {
            SettlementSubmissionOutcome::Success => "success",
            SettlementSubmissionOutcome::Revert => "revert",
            SettlementSubmissionOutcome::Timeout => "timeout",
            SettlementSubmissionOutcome::Cancel => "cancel",
            SettlementSubmissionOutcome::SimulationRevert => "simulationrevert",
            SettlementSubmissionOutcome::Disabled => "disabled",
            SettlementSubmissionOutcome::Failed => "failed",
        }
    }
}

#[derive(strum::EnumIter)]
pub enum SolverSimulationOutcome {
    Success,
    Failure,
    FailureOnLatest,
}

impl SolverSimulationOutcome {
    fn label(&self) -> &'static str {
        match self {
            SolverSimulationOutcome::Success => "success",
            SolverSimulationOutcome::Failure => "failure",
            SolverSimulationOutcome::FailureOnLatest => "failure_on_latest",
        }
    }
}

pub trait SolverMetrics: Send + Sync {
    fn orders_fetched(&self, orders: &[LimitOrder]);
    fn liquidity_fetched(&self, liquidity: &[Liquidity]);
    fn settlement_computed(&self, solver_type: &str, response: &str, start: Instant);
    fn order_settled(&self, order: &Order, solver: &str);
    fn settlement_simulation(&self, solver: &str, outcome: SolverSimulationOutcome);
    fn solver_run(&self, outcome: SolverRunOutcome, solver: &str);
    fn single_order_solver_succeeded(&self, solver: &str);
    fn single_order_solver_failed(&self, solver: &str);
    fn settlement_submitted(&self, outcome: SettlementSubmissionOutcome, solver: &str);
    fn settlement_access_list_saved_gas(&self, gas_saved: f64, sign: &str);
    fn settlement_revertable_status(&self, status: Revertable, solver: &str);
    fn orders_matched_but_not_settled(&self, count: usize);
    fn report_order_surplus(&self, surplus_diff: f64);
    fn runloop_completed(&self);
    fn complete_runloop_until_transaction(&self, duration: Duration);
    fn transaction_submission(&self, duration: Duration, strategy: &str);
    fn transaction_gas_price(&self, gas_price: U256);
}

// TODO add labeled interaction counter once we support more than one
// interaction
#[derive(prometheus_metric_storage::MetricStorage)]
struct Storage {
    /// Number of trades settled
    #[metric(name = "trade_counter_seconds", labels("solver_type", "trade_type"))]
    trade_counter: IntCounterVec,
    /// Counter for the number of seconds between creation and settlement of an
    /// order
    #[metric(name = "order_settlement_time_seconds", labels("order_type"))]
    order_settlement_time: IntCounterVec,
    /// Ms each solver takes to compute their solution
    #[metric(name = "computation_time_ms", labels("solver_type", "solution_type"))]
    solver_computation_time: IntCounterVec,
    /// Amount of orders labeled by liquidity type currently available to the
    /// solvers
    #[metric(name = "liquidity_gauge", labels("liquidity_type"))]
    liquidity: IntGaugeVec,
    /// Settlement simulation counts
    #[metric(labels("result", "solver_type"))]
    settlement_simulations: IntCounterVec,
    /// Settlement submission counts
    #[metric(labels("result", "solver_type"))]
    settlement_submissions: IntCounterVec,
    /// Settlement revertable status counts
    #[metric(labels("result", "solver_type"))]
    settlement_revertable_status: IntCounterVec,
    /// Saved gas by using access list for transaction submission
    #[metric(labels("sign"))]
    settlement_access_list_saved_gas: HistogramVec,
    /// Success/Failure counts
    #[metric(name = "solver_run", labels("result", "solver_type"))]
    solver_runs: IntCounterVec,
    /// Success/Failure counts
    #[metric(name = "single_order_solver", labels("result", "solver_type"))]
    single_order_solver_runs: IntCounterVec,
    /// Counter for the number of orders for which at least one solver computed
    /// an execution which was not chosen in this run-loop
    #[metric(name = "orders_matched_not_settled")]
    matched_but_unsettled_orders: IntCounter,
    /// Surplus ratio differences between winning and best settlement per order
    #[metric(name = "settlement_surplus_report", buckets(-1.0, -0.1, -0.01, -0.005, 0., 0.005, 0.01, 0.1, 1.0))]
    order_surplus_report: Histogram,
    /// Time a runloop that wants to submit a solution takes until the
    /// transaction submission starts.
    #[metric(name = "complete_runloop_until_transaction_seconds", buckets())]
    complete_runloop_until_transaction: Histogram,
    /// "Time it takes to submit a settlement transaction.
    #[metric(name = "transaction_submission_seconds", labels("strategy"), buckets())]
    transaction_submission: HistogramVec,
    /// Actual gas price used by settlement transaction.
    transaction_gas_price_gwei: Gauge,
}

pub struct Metrics {
    last_runloop_completed: Mutex<Instant>,
    metrics: &'static Storage,
}

impl Metrics {
    pub fn new() -> Result<Self> {
        Ok(Self {
            metrics: Storage::instance(global_metrics::get_metric_storage_registry()).unwrap(),
            last_runloop_completed: Mutex::new(Instant::now()),
        })
    }

    /// Initialize known to exist labels on solver related metrics to 0.
    ///
    /// Useful to make sure the prometheus metric exists for example for
    /// alerting.
    pub fn initialize_solver_metrics(&self, solver_names: &[&str]) {
        for solver in solver_names {
            for outcome in SolverSimulationOutcome::iter() {
                self.metrics
                    .settlement_simulations
                    .with_label_values(&[outcome.label(), solver])
                    .reset();
            }
            for outcome in SettlementSubmissionOutcome::iter() {
                self.metrics
                    .settlement_submissions
                    .with_label_values(&[outcome.label(), solver])
                    .reset();
            }
            for outcome in SolverRunOutcome::iter() {
                self.metrics
                    .solver_runs
                    .with_label_values(&[outcome.label(), solver])
                    .reset();
            }
        }
    }
}

impl SolverMetrics for Metrics {
    fn orders_fetched(&self, orders: &[LimitOrder]) {
        let user_orders = orders
            .iter()
            .filter(|order| !order.is_liquidity_order())
            .count();
        let liquidity_orders = orders.len() - user_orders;

        self.metrics
            .liquidity
            .with_label_values(&["UserOrder"])
            .set(user_orders as _);
        self.metrics
            .liquidity
            .with_label_values(&["LiquidityOrder"])
            .set(liquidity_orders as _);
    }

    fn liquidity_fetched(&self, liquidity: &[Liquidity]) {
        // Reset all gauges and start from scratch
        Liquidity::VARIANTS.iter().for_each(|label| {
            self.metrics.liquidity.with_label_values(&[label]).set(0);
        });
        liquidity.iter().for_each(|liquidity| {
            let label: &str = liquidity.into();
            self.metrics.liquidity.with_label_values(&[label]).inc();
        })
    }

    fn settlement_computed(&self, solver_type: &str, response: &str, start: Instant) {
        self.metrics
            .solver_computation_time
            .with_label_values(&[solver_type, response])
            .inc_by(
                Instant::now()
                    .duration_since(start)
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            )
    }

    fn order_settled(&self, order: &Order, solver: &str) {
        let time_to_settlement =
            chrono::offset::Utc::now().signed_duration_since(order.metadata.creation_date);
        let order_type = match order.metadata.class {
            OrderClass::Market => "user_order",
            OrderClass::Liquidity => "liquidity_order",
            OrderClass::Limit(_) => "limit_order",
        };
        self.metrics
            .trade_counter
            .with_label_values(&[solver, order_type])
            .inc();
        self.metrics
            .order_settlement_time
            .with_label_values(&[order_type])
            .inc_by(
                time_to_settlement
                    .num_seconds()
                    .try_into()
                    .unwrap_or_default(),
            )
    }

    fn settlement_simulation(&self, solver: &str, outcome: SolverSimulationOutcome) {
        self.metrics
            .settlement_simulations
            .with_label_values(&[outcome.label(), solver])
            .inc()
    }

    fn solver_run(&self, outcome: SolverRunOutcome, solver: &str) {
        self.metrics
            .solver_runs
            .with_label_values(&[outcome.label(), solver])
            .inc()
    }

    fn single_order_solver_succeeded(&self, solver: &str) {
        self.metrics
            .single_order_solver_runs
            .with_label_values(&["success", solver])
            .inc()
    }

    fn single_order_solver_failed(&self, solver: &str) {
        self.metrics
            .single_order_solver_runs
            .with_label_values(&["failure", solver])
            .inc()
    }

    fn settlement_submitted(&self, outcome: SettlementSubmissionOutcome, solver: &str) {
        self.metrics
            .settlement_submissions
            .with_label_values(&[outcome.label(), solver])
            .inc()
    }

    fn settlement_access_list_saved_gas(&self, gas_saved: f64, label: &str) {
        self.metrics
            .settlement_access_list_saved_gas
            .with_label_values(&[label])
            .observe(gas_saved);
    }

    fn orders_matched_but_not_settled(&self, count: usize) {
        self.metrics
            .matched_but_unsettled_orders
            .inc_by(count as u64);
    }

    fn report_order_surplus(&self, surplus_diff: f64) {
        self.metrics.order_surplus_report.observe(surplus_diff)
    }

    fn runloop_completed(&self) {
        *self
            .last_runloop_completed
            .lock()
            .expect("thread holding mutex panicked") = Instant::now();
    }

    fn complete_runloop_until_transaction(&self, duration: Duration) {
        self.metrics
            .complete_runloop_until_transaction
            .observe(duration.as_secs_f64());
    }

    fn transaction_submission(&self, duration: Duration, strategy: &str) {
        self.metrics
            .transaction_submission
            .with_label_values(&[strategy])
            .observe(duration.as_secs_f64());
    }

    fn transaction_gas_price(&self, gas_price: U256) {
        self.metrics
            .transaction_gas_price_gwei
            .set(gas_price.to_f64_lossy() / 1e9)
    }

    fn settlement_revertable_status(&self, status: Revertable, solver: &str) {
        let result = match status {
            Revertable::NoRisk => "no_risk",
            Revertable::HighRisk => "high_risk",
        };
        self.metrics
            .settlement_revertable_status
            .with_label_values(&[result, solver])
            .inc()
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

    fn settlement_computed(&self, _solver_type: &str, _response: &str, _start: Instant) {}

    fn order_settled(&self, _: &Order, _: &str) {}

    fn solver_run(&self, _: SolverRunOutcome, _: &str) {}

    fn single_order_solver_succeeded(&self, _: &str) {}

    fn single_order_solver_failed(&self, _: &str) {}

    fn settlement_submitted(&self, _: SettlementSubmissionOutcome, _: &str) {}

    fn settlement_revertable_status(&self, _: Revertable, _: &str) {}

    fn settlement_access_list_saved_gas(&self, _: f64, _: &str) {}

    fn orders_matched_but_not_settled(&self, _: usize) {}

    fn report_order_surplus(&self, _: f64) {}

    fn runloop_completed(&self) {}

    fn complete_runloop_until_transaction(&self, _: Duration) {}

    fn transaction_submission(&self, _: Duration, _: &str) {}

    fn transaction_gas_price(&self, _: U256) {}

    fn settlement_simulation(&self, _: &str, _: SolverSimulationOutcome) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_work() {
        let metrics = Metrics::new().unwrap();
        metrics.settlement_computed("asdf", "none", Instant::now());
        metrics.order_settled(&Default::default(), "test");
        metrics.settlement_simulation("test", SolverSimulationOutcome::Success);
        metrics.settlement_simulation("test", SolverSimulationOutcome::Failure);
        metrics.settlement_submitted(SettlementSubmissionOutcome::Success, "test");
        metrics.orders_matched_but_not_settled(20);
        metrics.initialize_solver_metrics(&["", "a"]);
    }
}
