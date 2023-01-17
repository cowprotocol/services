//! "Baseline" solver implementation.
//!
//! The baseline solver is a simple solver implementation that finds the best
//! path of at most length `max_hops + 1` over a set of on-chain liquidity. It
//! **does not** try to split large orders into multiple parts and route them
//! over separate paths.

use crate::{
    boundary,
    domain::{auction, eth, liquidity, order, solution},
};
use std::collections::HashSet;

use super::solution::Interaction;

pub struct Baseline {
    pub weth: eth::WethAddress,
    /// Set of tokens to additionally consider as intermediary hops when
    /// path-finding. This allows paths of the kind `TOKEN1 -> WETH -> TOKEN2`
    /// to be considered.
    pub base_tokens: HashSet<eth::TokenAddress>,
    /// Maximum number of hops that can be considered in a trading path. A hop
    /// is an intermediary token within a trading path. For example:
    /// - A value of 0 indicates that only a direct trade is allowed: `A -> B`
    /// - A value of 1 indicates that a single intermediary token can appear
    ///   within a trading path: `A -> B -> C`
    /// - A value of 2 indicates: `A -> B -> C -> D`
    /// - etc.
    pub max_hops: usize,
}

impl Baseline {
    /// Solves the specified auction, returning a vector of all possible
    /// solutions.
    pub fn solve(&self, auction: &auction::Auction) -> Vec<solution::Solution> {
        let boundary_solver =
            boundary::baseline::Solver::new(&self.weth, &self.base_tokens, &auction.liquidity);

        auction
            .orders
            .iter()
            .filter_map(|order| {
                let route =
                    boundary_solver.route(order::NonLiquidity::new(order)?, self.max_hops)?;

                Some(solution::Solution {
                    prices: solution::ClearingPrices::new([
                        (order.sell.token, route.output().amount),
                        (order.buy.token, route.input().amount),
                    ]),
                    trades: vec![solution::Trade::fill(order.clone())],
                    interactions: route
                        .segments
                        .iter()
                        .map(|segment| Interaction {
                            liquidity: segment.liquidity.clone(),
                            input: segment.input,
                            output: segment.output,
                        })
                        .collect(),
                })
            })
            .collect()
    }
}

/// A trading route.
pub struct Route<'a> {
    segments: Vec<Segment<'a>>,
}

/// A segment in a trading route.
pub struct Segment<'a> {
    pub liquidity: &'a liquidity::Liquidity,
    // TODO: There is no type-level guarantee here that both `input.token` and
    // `output.token` are valid for the liquidity in this segment. This is
    // unfortunate beacuse this type leaks out of this module (currently into
    // the `boundary::baseline` module) but should no longer need to be `pub`
    // once the `boundary::baseline` module gets refactored into the domain
    // logic, so I think it is fine for now.
    pub input: eth::Asset,
    pub output: eth::Asset,
}

impl<'a> Route<'a> {
    pub fn new(segments: Vec<Segment<'a>>) -> Option<Self> {
        if segments.is_empty() {
            return None;
        }
        Some(Self { segments })
    }

    fn input(&self) -> eth::Asset {
        self.segments[0].input
    }

    fn output(&self) -> eth::Asset {
        self.segments
            .last()
            .expect("route has at least one segment by construction")
            .output
    }
}
