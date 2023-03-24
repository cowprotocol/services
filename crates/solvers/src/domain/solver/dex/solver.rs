//! A simple solver that matches orders directly with swaps from the external
//! DEX and DEX aggregator APIs.

use {
    crate::{
        domain::{auction, dex::slippage, order, solution, solver::dex::fills::Fills},
        infra,
    },
    tracing::Instrument,
};

pub struct Dex {
    /// The DEX API client.
    dex: infra::dex::Dex,

    /// The slippage configuration to use for the solver.
    slippage: slippage::Limits,

    /// Helps to manage the strategy to fill orders (especially partially
    /// fillable orders).
    order_fill_handler: Fills,
}

impl Dex {
    pub fn new(dex: infra::dex::Dex, config: infra::config::dex::Config) -> Self {
        Self {
            dex,
            slippage: config.slippage,
            order_fill_handler: Fills::new(config.smallest_partial_fill),
        }
    }

    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        // TODO:
        // * order prioritization
        // * skip liquidity orders
        // * concurrency
        // * respecting auction deadline

        let prices = slippage::Prices::for_auction(&auction);

        let mut solutions = Vec::new();
        for order in auction.orders {
            let span = tracing::info_span!("solve", order = %order.uid);
            if let Some(solution) = self
                .solve_order(order, &prices, auction.gas_price)
                .instrument(span)
                .await
            {
                solutions.push(solution);
            }
        }

        self.order_fill_handler.collect_garbage();

        solutions
    }

    async fn solve_order(
        &self,
        order: order::Order,
        prices: &slippage::Prices,
        gas: auction::GasPrice,
    ) -> Option<solution::Solution> {
        let swap = {
            let order = self.order_fill_handler.dex_order(&order, prices)?;
            let slippage = self.slippage.relative(&order.amount(), prices);
            self.dex.swap(&order, &slippage, gas).await
        };

        let swap = match swap {
            Ok(swap) => swap,
            Err(err @ infra::dex::Error::NotFound) => {
                if order.partially_fillable {
                    // Only adjust the amount to try next if we are sure the API worked correctly
                    // yet still wasn't able to provide a swap.
                    self.order_fill_handler.reduce_next_try(order.uid);
                } else {
                    tracing::debug!(?err, "skipping order");
                }
                return None;
            }
            Err(err @ infra::dex::Error::OrderNotSupported) => {
                tracing::debug!(?err, "skipping order");
                return None;
            }
            Err(infra::dex::Error::Other(err)) => {
                tracing::warn!(?err, "failed to get swap");
                return None;
            }
        };

        let Some(solution) = swap.into_solution(order) else {
            tracing::debug!("no solution for swap");
            return None;
        };

        tracing::debug!("solved");
        Some(solution)
    }
}
