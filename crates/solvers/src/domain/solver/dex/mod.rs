//! A simple solver that matches orders directly with swaps from the external
//! DEX and DEX aggregator APIs.

use {
    crate::{
        domain,
        domain::{auction, dex::slippage, order, solution, solver::dex::fills::Fills},
        infra,
    },
    futures::{future, stream, StreamExt},
    std::num::NonZeroUsize,
    tracing::Instrument,
};

mod fills;

pub struct Dex {
    /// The DEX API client.
    dex: infra::dex::Dex,

    /// The slippage configuration to use for the solver.
    slippage: slippage::Limits,

    /// The number of concurrent requests to make.
    concurrent_requests: NonZeroUsize,

    /// Helps to manage the strategy to fill orders (especially partially
    /// fillable orders).
    fills: Fills,

    /// Parameters used to calculate the revert risk of a solution.
    risk: domain::Risk,
}

impl Dex {
    pub fn new(dex: infra::dex::Dex, config: infra::config::dex::Config) -> Self {
        Self {
            dex,
            slippage: config.slippage,
            concurrent_requests: config.concurrent_requests,
            fills: Fills::new(config.smallest_partial_fill),
            risk: config.risk,
        }
    }

    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        let mut solutions = Vec::new();
        let solve_orders = async {
            let mut stream = self.solution_stream(&auction);
            while let Some(solution) = stream.next().await {
                solutions.push(solution);
            }
        };

        let deadline = auction.deadline.remaining().unwrap_or_default();
        if tokio::time::timeout(deadline, solve_orders).await.is_err() {
            tracing::debug!("reached deadline; stopping to solve");
        }

        self.fills.collect_garbage();

        solutions
    }

    fn solution_stream<'a>(
        &'a self,
        auction: &'a auction::Auction,
    ) -> impl stream::Stream<Item = solution::Solution> + 'a {
        stream::iter(auction.orders.iter().filter_map(order::UserOrder::new))
            .map(|order| {
                let span = tracing::info_span!("solve", order = %order.get().uid);
                self.solve_order(order, &auction.tokens, auction.gas_price)
                    .instrument(span)
            })
            .buffer_unordered(self.concurrent_requests.get())
            .filter_map(future::ready)
    }

    async fn solve_order(
        &self,
        order: order::UserOrder<'_>,
        tokens: &auction::Tokens,
        gas_price: auction::GasPrice,
    ) -> Option<solution::Solution> {
        let order = order.get();
        let swap = {
            let order = self.fills.dex_order(order, tokens)?;
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
        let score =
            solution::Score::RiskAdjusted(self.risk.success_probability(swap.gas, gas_price, 1));
        let Some(solution) = swap.into_solution(order.clone(), gas_price, sell, score) else {
            tracing::debug!("no solution for swap");
            return None;
        };

        tracing::debug!("solved");
        // Maybe some liquidity appeared that enables a bigger fill.
        self.fills.increase_next_try(uid);

        Some(solution.with_buffers_internalizations(tokens))
    }
}
