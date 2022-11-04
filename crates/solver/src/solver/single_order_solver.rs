use crate::{
    driver::solver_settlements::merge_settlements,
    liquidity::LimitOrder,
    metrics::SolverMetrics,
    settlement::Settlement,
    solver::{Auction, Solver},
};
use anyhow::{Error, Result};
use ethcontract::Account;
use primitive_types::U256;
use rand::prelude::SliceRandom;
use std::{collections::VecDeque, sync::Arc, time::Duration};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
/// Implementations of this trait know how to settle a single limit order (not taking advantage of batching multiple orders together)
pub trait SingleOrderSolving: Send + Sync + 'static {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        auction: &Auction,
    ) -> Result<Option<Settlement>, SettlementError>;

    /// Solver's account that should be used to submit settlements.
    fn account(&self) -> &Account;

    /// Displayable name of the solver. Defaults to the type name.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub struct SingleOrderSolver {
    inner: Box<dyn SingleOrderSolving>,
    metrics: Arc<dyn SolverMetrics>,
    max_merged_settlements: usize,
    max_settlements_per_solver: usize,
}

impl SingleOrderSolver {
    pub fn new(
        inner: Box<dyn SingleOrderSolving>,
        metrics: Arc<dyn SolverMetrics>,
        max_settlements_per_solver: usize,
        max_merged_settlements: usize,
    ) -> Self {
        Self {
            inner,
            metrics,
            max_merged_settlements,
            max_settlements_per_solver,
        }
    }
}

#[async_trait::async_trait]
impl Solver for SingleOrderSolver {
    async fn solve(&self, auction: Auction) -> Result<Vec<Settlement>> {
        let mut orders = auction.orders.clone();

        // Randomize which orders we start with to prevent us getting stuck on bad orders.
        orders.shuffle(&mut rand::thread_rng());

        let mut orders = orders
            .into_iter()
            .filter(|order| !order.is_liquidity_order)
            .collect::<VecDeque<_>>();
        let mut settlements = Vec::new();
        let settle = async {
            while let Some(order) = orders.pop_front() {
                match self.inner.try_settle_order(order.clone(), &auction).await {
                    Ok(settlement) => {
                        self.metrics
                            .single_order_solver_succeeded(self.inner.name());
                        settlements.extend(settlement)
                    }
                    Err(err) => {
                        let name = self.inner.name();
                        self.metrics.single_order_solver_failed(name);
                        if err.retryable {
                            tracing::warn!("Solver {} retryable error: {:?}", name, &err.inner);
                            orders.push_back(order);
                        } else {
                            tracing::warn!("Solver {} error: {:?}", name, &err.inner);
                        }
                    }
                }
            }
        };

        // Subtract a small amount of time to ensure that the driver doesn't reach the deadline first.
        let _ = tokio::time::timeout_at((auction.deadline - Duration::from_secs(1)).into(), settle)
            .await;

        // Keep at most this many settlements. This is important in case where a solver produces
        // a large number of settlements which would hold up the driver logic when simulating
        // them.
        // Shuffle first so that in the case a buggy solver keeps returning some amount of
        // invalid settlements first we have a chance to make progress.
        settlements.shuffle(&mut rand::thread_rng());
        settlements.truncate(self.max_settlements_per_solver);

        merge_settlements(
            self.max_merged_settlements,
            &auction.external_prices,
            &mut settlements,
        );

        Ok(settlements)
    }

    fn account(&self) -> &Account {
        self.inner.account()
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }
}

#[derive(Debug)]
pub struct SettlementError {
    pub inner: anyhow::Error,
    pub retryable: bool,
}

impl From<anyhow::Error> for SettlementError {
    fn from(err: Error) -> Self {
        SettlementError {
            inner: err,
            retryable: false,
        }
    }
}

// Used by the single order solvers to verify that the response respects the order price.
// We have also observed that a 0x buy order did not respect the queried buy amount so verifying
// just the price or verifying just one component of the price (sell amount for buy orders, buy
// amount for sell orders) is not enough.
pub fn execution_respects_order(
    order: &LimitOrder,
    executed_sell_amount: U256,
    executed_buy_amount: U256,
) -> bool {
    // note: This would be different for partially fillable orders but LimitOrder does currently not
    // contain the remaining fill amount.
    executed_sell_amount <= order.sell_amount && executed_buy_amount >= order.buy_amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{liquidity::tests::CapturingSettlementHandler, metrics::NoopMetrics};
    use anyhow::anyhow;
    use model::order::OrderKind;
    use std::sync::Arc;

    fn test_solver(inner: MockSingleOrderSolving) -> SingleOrderSolver {
        SingleOrderSolver {
            inner: Box::new(inner),
            metrics: Arc::new(NoopMetrics::default()),
            max_merged_settlements: 5,
            max_settlements_per_solver: 5,
        }
    }

    #[tokio::test]
    async fn uses_inner_solver() {
        let mut inner = MockSingleOrderSolving::new();
        inner
            .expect_try_settle_order()
            .times(2)
            .returning(|_, _| Ok(Some(Settlement::new(Default::default()))));
        inner.expect_name().returning(|| "Mock Solver");

        let solver = test_solver(inner);
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            settlement_handling: handler.clone(),
            ..Default::default()
        };
        let orders = vec![
            LimitOrder {
                id: 0.into(),
                ..order.clone()
            },
            LimitOrder {
                id: 1.into(),
                ..order.clone()
            },
        ];

        let settlements = solver
            .solve(Auction {
                orders,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(settlements.len(), 3);
    }

    #[tokio::test]
    async fn retries_retryable() {
        let mut inner = MockSingleOrderSolving::new();
        inner.expect_name().return_const("");
        let mut call_count = 0u32;
        inner
            .expect_try_settle_order()
            .times(2)
            .returning(move |_, _| {
                dbg!();
                let result = match call_count {
                    0 => Err(SettlementError {
                        inner: anyhow!(""),
                        retryable: true,
                    }),
                    1 => Ok(None),
                    _ => unreachable!(),
                };
                call_count += 1;
                result
            });

        let solver = test_solver(inner);
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            settlement_handling: handler.clone(),
            ..Default::default()
        };
        solver
            .solve(Auction {
                orders: vec![order],
                ..Default::default()
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn does_not_retry_unretryable() {
        let mut inner = MockSingleOrderSolving::new();
        inner.expect_name().return_const("");
        inner.expect_try_settle_order().times(1).returning(|_, _| {
            Err(SettlementError {
                inner: anyhow!(""),
                retryable: false,
            })
        });

        let solver = test_solver(inner);
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            settlement_handling: handler.clone(),
            ..Default::default()
        };
        solver
            .solve(Auction {
                orders: vec![order],
                ..Default::default()
            })
            .await
            .unwrap();
    }

    #[test]
    fn execution_respects_order_() {
        let order = LimitOrder {
            kind: OrderKind::Sell,
            sell_amount: 10.into(),
            buy_amount: 10.into(),
            ..Default::default()
        };
        assert!(execution_respects_order(&order, 10.into(), 11.into(),));
        assert!(!execution_respects_order(&order, 10.into(), 9.into(),));
        // Unexpectedly the executed sell amount is less than the real sell order for a fill kill
        // order but we still get enough buy token.
        assert!(execution_respects_order(&order, 9.into(), 10.into(),));
        // Price is respected but order is partially filled.
        assert!(!execution_respects_order(&order, 9.into(), 9.into(),));

        let order = LimitOrder {
            kind: OrderKind::Buy,
            ..order
        };
        assert!(execution_respects_order(&order, 9.into(), 10.into(),));
        assert!(!execution_respects_order(&order, 11.into(), 10.into(),));
        // Unexpectedly get more buy amount but sell amount is still respected.
        assert!(execution_respects_order(&order, 10.into(), 11.into(),));
        // Price is respected but order is partially filled.
        assert!(!execution_respects_order(&order, 9.into(), 9.into(),));
    }
}
