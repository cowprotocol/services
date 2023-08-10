//! A simple solver that matches orders directly with swaps from the external
//! DEX and DEX aggregator APIs.

mod fills;

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
    fills: Fills,
}

impl Dex {
    pub fn new(dex: infra::dex::Dex, config: infra::config::dex::Config) -> Self {
        Self {
            dex,
            slippage: config.slippage,
            fills: Fills::new(config.smallest_partial_fill),
        }
    }

    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        // TODO:
        // * order prioritization
        // * skip liquidity orders

        let futures = auction
            .orders
            .into_iter()
            .map(|order| {
                let deadline = auction
                    .deadline
                    .signed_duration_since(chrono::Utc::now())
                    .to_std()
                    .unwrap_or_default();
                let tokens = auction.tokens.clone();
                let order_uid = order.uid;
                async move {
                    let span = tracing::info_span!("solve", order = %order_uid);
                    match tokio::time::timeout(
                        deadline,
                        self.solve_order(order, &tokens, auction.gas_price),
                    )
                    .instrument(span)
                    .await
                    {
                        Ok(inner) => inner,
                        Err(_) => {
                            tracing::debug!(order = %order_uid, "skipping order due to timeout");
                            None
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

        let solutions = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        self.fills.collect_garbage();

        solutions
    }

    async fn solve_order(
        &self,
        order: order::Order,
        tokens: &auction::Tokens,
        gas_price: auction::GasPrice,
    ) -> Option<solution::Solution> {
        let swap = {
            let order = self.fills.dex_order(&order, tokens)?;
            let slippage = self.slippage.relative(&order.amount(), tokens);
            self.dex.swap(&order, &slippage, tokens, gas_price).await
        };

        let swap = match swap {
            Ok(swap) => swap,
            Err(err @ infra::dex::Error::NotFound) => {
                if order.partially_fillable {
                    // Only adjust the amount to try next if we are sure the API worked correctly
                    // yet still wasn't able to provide a swap.
                    self.fills.reduce_next_try(order.uid);
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

        let uid = order.uid;
        let sell = tokens.reference_price(&order.sell.token);
        let Some(solution) = swap.into_solution(order, gas_price, sell) else {
            tracing::debug!("no solution for swap");
            return None;
        };

        tracing::debug!("solved");
        // Maybe some liquidity appeared that enables a bigger fill.
        self.fills.increase_next_try(uid);

        Some(solution)
    }
}
