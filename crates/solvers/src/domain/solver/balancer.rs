//! A simple DEX solver that matches orders directly with swaps from the
//! Balancer SOR API.

use crate::{
    domain::{
        auction,
        dex::{self, slippage},
        solution,
    },
    infra,
};

pub struct Balancer {
    /// The Balancer SOR API client.
    pub sor: infra::dex::balancer::Sor,

    /// The slippage configuration to use for the solver.
    pub slippage: slippage::Limits,
}

impl Balancer {
    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        // TODO: order prioritization, skip liquidity orders, concurrency.
        let prices = slippage::Prices::for_auction(&auction);

        let mut solutions = Vec::new();
        for order in auction.orders {
            let query = dex::Order::new(&order);

            let slippage = self.slippage.relative(&query.amount(), &prices);
            let swap = match self.sor.swap(&query, &slippage, auction.gas_price).await {
                Ok(value) => value,
                Err(infra::dex::balancer::Error::NotFound) => continue,
                Err(err) => {
                    tracing::warn!(?err, "failed to get swap");
                    continue;
                }
            };

            if let Some(solution) = swap.into_solution(order) {
                solutions.push(solution);
            }
        }

        solutions
    }
}
