use crate::metrics::SolverMetrics;
use crate::{
    liquidity::LimitOrder,
    settlement::Settlement,
    solver::{Auction, Solver},
};
use anyhow::{Error, Result};
use ethcontract::Account;
use rand::prelude::SliceRandom;
use std::sync::Arc;
use std::{collections::VecDeque, time::Duration};

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

pub struct SingleOrderSolver<I> {
    inner: I,
    metrics: Arc<dyn SolverMetrics>,
}

impl<I: SingleOrderSolving> SingleOrderSolver<I> {
    pub fn new(inner: I, metrics: Arc<dyn SolverMetrics>) -> Self {
        Self { inner, metrics }
    }
}

#[async_trait::async_trait]
impl<I: SingleOrderSolving> Solver for SingleOrderSolver<I> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::tests::CapturingSettlementHandler;
    use crate::metrics::NoopMetrics;
    use anyhow::anyhow;
    use std::sync::Arc;

    #[tokio::test]
    async fn uses_inner_solver() {
        let mut inner = MockSingleOrderSolving::new();
        inner
            .expect_try_settle_order()
            .times(2)
            .returning(|_, _| Ok(Some(Settlement::new(Default::default()))));
        inner.expect_name().returning(|| "Mock Solver");

        let solver: SingleOrderSolver<_> =
            SingleOrderSolver::new(inner, Arc::new(NoopMetrics::default()));
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            settlement_handling: handler.clone(),
            ..Default::default()
        };
        let orders = vec![
            LimitOrder {
                id: 0.to_string(),
                ..order.clone()
            },
            LimitOrder {
                id: 1.to_string(),
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
        assert_eq!(settlements.len(), 2);
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

        let solver: SingleOrderSolver<_> =
            SingleOrderSolver::new(inner, Arc::new(NoopMetrics::default()));
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

        let solver: SingleOrderSolver<_> =
            SingleOrderSolver::new(inner, Arc::new(NoopMetrics::default()));
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
}
