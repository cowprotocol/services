use crate::{
    driver::solver_settlements::RatedSettlement, metrics::SolverMetrics, settlement::Settlement,
    solver::Solver,
};
use ethcontract::H160;
use model::order::OrderUid;
use num::{BigRational, ToPrimitive, Zero};
use shared::conversions::U256Ext;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter},
    sync::Arc,
};

pub fn report_matched_but_not_settled(
    metrics: &dyn SolverMetrics,
    (_, winning_solution): &(Arc<dyn Solver>, RatedSettlement),
    alternative_settlements: &[(Arc<dyn Solver>, RatedSettlement)],
) {
    let submitted_orders: HashSet<_> = winning_solution
        .settlement
        .encoder
        .order_trades()
        .iter()
        .map(|order_trade| order_trade.trade.order.metadata.uid)
        .collect();
    let other_matched_orders: HashSet<_> = alternative_settlements
        .iter()
        .flat_map(|(_, solution)| solution.settlement.encoder.order_trades().to_vec())
        .map(|order_trade| order_trade.trade.order.metadata.uid)
        .collect();
    let matched_but_not_settled: HashSet<_> = other_matched_orders
        .difference(&submitted_orders)
        .copied()
        .collect();

    if !matched_but_not_settled.is_empty() {
        tracing::debug!(
            ?matched_but_not_settled,
            "some orders were matched but not settled"
        );
    }

    metrics.orders_matched_but_not_settled(matched_but_not_settled.len());
}

#[derive(Clone)]
struct SurplusInfo {
    solver_name: String,
    ratio: BigRational,
}

impl SurplusInfo {
    fn is_better_than(&self, other: &Self) -> bool {
        self.ratio > other.ratio
    }
}

impl Display for SurplusInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Surplus {{solver: {}, ratio: {:.2e} }}",
            self.solver_name,
            self.ratio.to_f64().unwrap_or(f64::NAN)
        )
    }
}

fn get_prices(settlement: &Settlement) -> HashMap<H160, BigRational> {
    settlement
        .clearing_prices()
        .iter()
        .map(|(token, price)| (*token, price.to_big_rational()))
        .collect::<HashMap<_, _>>()
}

/// Record metric with surplus achieved in winning settlement
/// vs that which was unrealized in other feasible solutions.
pub fn report_alternative_settlement_surplus(
    metrics: &dyn SolverMetrics,
    winning_settlement: &(Arc<dyn Solver>, RatedSettlement),
    alternative_settlements: &[(Arc<dyn Solver>, RatedSettlement)],
) {
    let (winning_solver, submitted) = winning_settlement;
    let submitted_prices = get_prices(&submitted.settlement);
    let submitted_surplus: HashMap<_, _> = submitted
        .settlement
        .encoder
        .order_trades()
        .iter()
        .map(|order_trade| {
            let sell_token_price = &submitted_prices[&order_trade.trade.order.data.sell_token];
            let buy_token_price = &submitted_prices[&order_trade.trade.order.data.buy_token];
            (
                order_trade.trade.order.metadata.uid,
                SurplusInfo {
                    solver_name: winning_solver.name().to_string(),
                    ratio: order_trade
                        .trade
                        .surplus_ratio(sell_token_price, buy_token_price)
                        .unwrap_or_else(BigRational::zero),
                },
            )
        })
        .collect();

    let best_alternative = best_surplus_by_order(alternative_settlements);
    for (order_id, submitted) in submitted_surplus.iter() {
        if let Some(alternative) = best_alternative.get(order_id) {
            metrics.report_order_surplus(
                (&submitted.ratio - &alternative.ratio)
                    .to_f64()
                    .unwrap_or_default(),
            );
            if alternative.is_better_than(submitted) {
                tracing::debug!(
                    ?order_id, %submitted, %alternative,
                    "submission surplus worse than lower ranked settlement",
                );
            }
        }
    }
}

fn best_surplus_by_order(
    settlements: &[(Arc<dyn Solver>, RatedSettlement)],
) -> HashMap<OrderUid, SurplusInfo> {
    let mut best_surplus: HashMap<OrderUid, SurplusInfo> = HashMap::new();
    for (solver, solution) in settlements.iter() {
        let trades = solution.settlement.encoder.order_trades();
        let clearing_prices = get_prices(&solution.settlement);
        for order_trade in trades {
            let order_id = order_trade.trade.order.metadata.uid;
            let sell_token_price = &clearing_prices[&order_trade.trade.order.data.sell_token];
            let buy_token_price = &clearing_prices[&order_trade.trade.order.data.buy_token];
            let surplus = SurplusInfo {
                solver_name: solver.name().to_string(),
                ratio: order_trade
                    .trade
                    .surplus_ratio(sell_token_price, buy_token_price)
                    .unwrap_or_else(BigRational::zero),
            };
            best_surplus
                .entry(order_id)
                .and_modify(|entry| {
                    if entry.ratio < surplus.ratio {
                        *entry = surplus.clone();
                    }
                })
                .or_insert(surplus);
        }
    }
    best_surplus
}
