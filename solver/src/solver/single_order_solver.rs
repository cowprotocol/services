use crate::{
    liquidity::LimitOrder,
    settlement::Settlement,
    solver::{Auction, Solver},
};
use anyhow::{Error, Result};
use ethcontract::Account;
use rand::prelude::SliceRandom;
use std::{collections::VecDeque, time::Duration};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
/// Implementations of this trait know how to settle a single limit order (not taking advantage of batching multiple orders together)
pub trait SingleOrderSolving {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
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
}

impl<I: SingleOrderSolving> From<I> for SingleOrderSolver<I> {
    fn from(inner: I) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl<I: SingleOrderSolving + Send + Sync + 'static> Solver for SingleOrderSolver<I> {
    async fn solve(
        &self,
        Auction {
            mut orders,
            deadline,
            ..
        }: Auction,
    ) -> Result<Vec<Settlement>> {
        // Randomize which orders we start with to prevent us getting stuck on bad orders.
        orders.shuffle(&mut rand::thread_rng());

        let mut orders = orders.into_iter().collect::<VecDeque<_>>();
        let mut settlements = Vec::new();
        let settle = async {
            while let Some(order) = orders.pop_front() {
                match self.inner.try_settle_order(order.clone()).await {
                    Ok(settlement) => settlements.extend(settlement),
                    Err(err) => {
                        let name = self.inner.name();
                        if err.retryable {
                            tracing::warn!("Solver {} benign error: {:?}", name, &err);
                            orders.push_back(order);
                        } else {
                            tracing::error!("Solver {} hard error: {:?}", name, &err);
                        }
                    }
                }
            }
        };

        // Subtract a small amount of time to ensure that the driver doesn't reach the deadline first.
        let _ = tokio::time::timeout_at((deadline - Duration::from_secs(1)).into(), settle).await;
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
    use anyhow::anyhow;
    use std::sync::Arc;

    #[tokio::test]
    async fn uses_inner_solver() {
        let mut inner = MockSingleOrderSolving::new();
        inner
            .expect_try_settle_order()
            .times(2)
            .returning(|_| Ok(Some(Settlement::new(Default::default()))));

        let solver: SingleOrderSolver<_> = inner.into();
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            id: Default::default(),
            sell_token: Default::default(),
            buy_token: Default::default(),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: Default::default(),
            fee_amount: Default::default(),
            settlement_handling: handler.clone(),
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
            .returning(move |_| {
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

        let solver: SingleOrderSolver<_> = inner.into();
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            id: Default::default(),
            sell_token: Default::default(),
            buy_token: Default::default(),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: Default::default(),
            fee_amount: Default::default(),
            settlement_handling: handler.clone(),
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
        inner.expect_try_settle_order().times(1).returning(|_| {
            Err(SettlementError {
                inner: anyhow!(""),
                retryable: false,
            })
        });

        let solver: SingleOrderSolver<_> = inner.into();
        let handler = Arc::new(CapturingSettlementHandler::default());
        let order = LimitOrder {
            id: Default::default(),
            sell_token: Default::default(),
            buy_token: Default::default(),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: Default::default(),
            fee_amount: Default::default(),
            settlement_handling: handler.clone(),
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
