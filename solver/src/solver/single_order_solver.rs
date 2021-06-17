use anyhow::Result;
use futures::future;
use rand::prelude::SliceRandom;

use crate::{
    liquidity::{LimitOrder, Liquidity},
    settlement::Settlement,
    solver::Solver,
};

#[async_trait::async_trait]
/// Implementations of this trait know how to settle a single limit order (not taking advantage of batching multiple orders together)
pub trait SingleOrderSolving {
    /// Return a settlement for the given limit order (if possible)
    async fn settle_order(&self, order: LimitOrder) -> Result<Option<Settlement>>;

    fn name(&self) -> &'static str;
}

/// Maximum number of sell orders to consider for settlements.
///
/// This is mostly out of concern to avoid rate limiting and because
/// requests may a non-trivial amount of time.
const MAX_SETTLEMENTS: usize = 5;

pub struct SingleOrderSolver<I> {
    inner: I,
}
impl<I: SingleOrderSolving> From<I> for SingleOrderSolver<I> {
    fn from(inner: I) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl<I: SingleOrderSolving + Send + Sync> Solver for SingleOrderSolver<I> {
    async fn solve(&self, liquidity: Vec<Liquidity>, _gas_price: f64) -> Result<Vec<Settlement>> {
        let mut orders = liquidity
            .into_iter()
            .filter_map(|liquidity| match liquidity {
                Liquidity::Limit(order) => Some(order),
                _ => None,
            })
            .collect::<Vec<_>>();

        // Randomize which orders we take, this prevents this solver "getting
        // stuck" on bad orders.
        if orders.len() > MAX_SETTLEMENTS {
            orders.shuffle(&mut rand::thread_rng());
        }

        let settlements = future::join_all(
            orders
                .into_iter()
                .take(MAX_SETTLEMENTS)
                .map(|order| self.inner.settle_order(order)),
        )
        .await;

        Ok(settlements
            .into_iter()
            .filter_map(|settlement| match settlement {
                Ok(Some(settlement)) => Some(settlement),
                Ok(None) => None,
                Err(err) => {
                    // It could be that the inner solver can't match an order and would
                    // return an error for whatever reason. In that case, we want
                    // to continue trying to solve for other orders.
                    tracing::error!("Inner solver error: {:?}", err);
                    None
                }
            })
            .collect())
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }
}
