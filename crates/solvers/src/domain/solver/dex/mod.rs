//! A simple solver that matches orders directly with swaps from the external
//! DEX and DEX aggregator APIs.

use {
    crate::{
        boundary::rate_limiter::{RateLimiter, RateLimiterError},
        domain::{
            self,
            auction,
            dex::{self, slippage},
            order::{self, Order},
            solution,
            solver::dex::fills::Fills,
        },
        infra,
    },
    futures::{future, stream, FutureExt, StreamExt},
    std::num::NonZeroUsize,
    tracing::Instrument,
};

mod fills;

pub struct Dex {
    /// The DEX API client.
    dex: infra::dex::Dex,

    /// A DEX swap gas simulator for computing limit order fees.
    simulator: infra::dex::Simulator,

    /// The slippage configuration to use for the solver.
    slippage: slippage::Limits,

    /// The number of concurrent requests to make.
    concurrent_requests: NonZeroUsize,

    /// Helps to manage the strategy to fill orders (especially partially
    /// fillable orders).
    fills: Fills,

    /// Parameters used to calculate the revert risk of a solution.
    risk: domain::Risk,

    /// Handles 429 Too Many Requests error with a retry mechanism
    rate_limiter: RateLimiter,
}

impl Dex {
    pub fn new(dex: infra::dex::Dex, config: infra::config::dex::Config) -> Self {
        let rate_limiter = RateLimiter::new(config.rate_limiting_strategy, "dex_api".to_string());
        Self {
            dex,
            simulator: infra::dex::Simulator::new(
                &config.node_url,
                config.contracts.settlement,
                config.contracts.authenticator,
            ),
            slippage: config.slippage,
            concurrent_requests: config.concurrent_requests,
            fills: Fills::new(config.smallest_partial_fill),
            risk: config.risk,
            rate_limiter,
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
            .enumerate()
            .map(|(i, order)| {
                let span = tracing::info_span!("solve", order = %order.get().uid);
                self.solve_order(order, &auction.tokens, auction.gas_price)
                    .map(move |solution| solution.map(|s| s.with_id(solution::Id(i as u64))))
                    .instrument(span)
            })
            .buffer_unordered(self.concurrent_requests.get())
            .filter_map(future::ready)
    }

    async fn try_solve(
        &self,
        order: &Order,
        dex_order: &dex::Order,
        tokens: &auction::Tokens,
        gas_price: auction::GasPrice,
    ) -> Option<dex::Swap> {
        let dex_err_handler = |err: infra::dex::Error| {
            infra::metrics::solve_error(err.format_variant());
            match &err {
                err @ infra::dex::Error::NotFound => {
                    if order.partially_fillable {
                        // Only adjust the amount to try next if we are sure the API
                        // worked
                        // correctly yet still wasn't able to provide a
                        // swap.
                        self.fills.reduce_next_try(order.uid);
                    } else {
                        tracing::debug!(?err, "skipping order");
                    }
                }
                err @ infra::dex::Error::OrderNotSupported => {
                    tracing::debug!(?err, "skipping order")
                }
                err @ infra::dex::Error::RateLimited => {
                    tracing::debug!(?err, "encountered rate limit")
                }
                infra::dex::Error::Other(err) => {
                    tracing::warn!(?err, "failed to get swap")
                }
            }
            err
        };
        let swap = async {
            let slippage = self.slippage.relative(&dex_order.amount(), tokens);
            self.dex
                .swap(dex_order, &slippage, tokens, gas_price)
                .await
                .map_err(dex_err_handler)
        };
        self.rate_limiter
            .execute_with_back_off(swap, |result| {
                matches!(result, Err(infra::dex::Error::RateLimited))
            })
            .await
            .map_err(|err| match err {
                RateLimiterError::RateLimited => infra::dex::Error::RateLimited,
            })
            .and_then(|result| result)
            .ok()
    }

    async fn solve_order(
        &self,
        order: order::UserOrder<'_>,
        tokens: &auction::Tokens,
        gas_price: auction::GasPrice,
    ) -> Option<solution::Solution> {
        let order = order.get();
        let dex_order = self.fills.dex_order(order, tokens)?;
        let swap = self.try_solve(order, &dex_order, tokens, gas_price).await?;
        let sell = tokens.reference_price(&order.sell.token);
        let Some(solution) = swap
            .into_solution(order.clone(), gas_price, sell, &self.risk, &self.simulator)
            .await
        else {
            tracing::debug!("no solution for swap");
            return None;
        };

        tracing::debug!("solved");
        // Maybe some liquidity appeared that enables a bigger fill.
        self.fills.increase_next_try(order.uid);

        Some(solution.with_buffers_internalizations(tokens))
    }
}
