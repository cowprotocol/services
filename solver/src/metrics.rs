use std::{convert::TryInto, time::Instant};

use anyhow::Result;
use model::order::Order;
use prometheus::{IntCounter, IntCounterVec, IntGaugeVec, Opts, Registry};
use strum::{AsStaticRef, VariantNames};

use crate::liquidity::Liquidity;

pub trait SolverMetrics {
    fn liquidity_fetched(&self, liquidity: &[Liquidity]);
    fn settlement_computed(&self, solver_type: &str, start: Instant);
    fn order_settled(&self, order: &Order);
}

// TODO add labeled interaction counter once we support more than one interaction
pub struct Metrics {
    trade_counter: IntCounter,
    order_settlement_time: IntCounter,
    solver_computation_time: IntCounterVec,
    liquidity: IntGaugeVec,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Result<Self> {
        let trade_counter =
            IntCounter::new("gp_v2_solver_trade_counter", "Number of trades settled")?;
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

        Ok(Self {
            trade_counter,
            order_settlement_time,
            solver_computation_time,
            liquidity,
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

    fn order_settled(&self, order: &Order) {
        let time_to_settlement =
            chrono::offset::Utc::now().signed_duration_since(order.order_meta_data.creation_date);
        self.trade_counter.inc();
        self.order_settlement_time.inc_by(
            time_to_settlement
                .num_seconds()
                .try_into()
                .unwrap_or_default(),
        )
    }
}

#[derive(Default)]
pub struct NoopMetrics {}

impl SolverMetrics for NoopMetrics {
    fn liquidity_fetched(&self, _liquidity: &[Liquidity]) {}
    fn settlement_computed(&self, _solver_type: &str, _start: Instant) {}
    fn order_settled(&self, _order: &Order) {}
}
