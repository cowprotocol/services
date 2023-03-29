//! "Baseline" solver implementation.
//!
//! The baseline solver is a simple solver implementation that finds the best
//! path of at most length `max_hops + 1` over a set of on-chain liquidity. It
//! **does not** try to split large orders into multiple parts and route them
//! over separate paths.

use {
    crate::{
        boundary,
        domain::{auction, eth, liquidity, order, solution},
    },
    ethereum_types::U256,
    std::collections::HashSet,
};

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
    /// The maximum number of attempts to solve a partially fillable order.
    /// Basically we continuously halve the amount to execute until we find a
    /// valid solution or exceed this count.
    pub max_partial_attempts: usize,
}

impl Baseline {
    /// Solves the specified auction, returning a vector of all possible
    /// solutions.
    pub fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        let boundary_solver =
            boundary::baseline::Solver::new(&self.weth, &self.base_tokens, &auction.liquidity);

        auction
            .orders
            .iter()
            .filter_map(|order| {
                let sell_token = auction.tokens.get(&order.sell.token)?.reference_price?;
                self.requests_for_order(order::NonLiquidity::new(order)?)
                    .find_map(|request| {
                        tracing::trace!(?request, "finding route");

                        let route = boundary_solver.route(request, self.max_hops)?;
                        let interactions = route
                            .segments
                            .iter()
                            .map(|segment| {
                                solution::Interaction::Liquidity(solution::LiquidityInteraction {
                                    liquidity: segment.liquidity.clone(),
                                    input: segment.input,
                                    output: segment.output,
                                    // TODO does the baseline solver know about this
                                    // optimization?
                                    internalize: false,
                                })
                            })
                            .collect();

                        solution::Single {
                            order: order.clone(),
                            input: route.input(),
                            output: route.output(),
                            interactions,
                            gas: route.gas(),
                        }
                        .into_solution(auction.gas_price, sell_token)
                    })
            })
            .collect()
    }

    fn requests_for_order(&self, order: order::NonLiquidity) -> impl Iterator<Item = Request> {
        let order::Order {
            sell, buy, side, ..
        } = order.get().clone();

        let n = if order.get().partially_fillable {
            self.max_partial_attempts
        } else {
            1
        };

        (0..n).map(move |i| {
            let divisor = U256::one() << i;
            Request {
                sell: eth::Asset {
                    token: sell.token,
                    amount: sell.amount / divisor,
                },
                buy: eth::Asset {
                    token: buy.token,
                    amount: buy.amount / divisor,
                },
                side,
            }
        })
    }
}

/// A baseline routing request.
#[derive(Debug)]
pub struct Request {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: order::Side,
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
    // unfortunate because this type leaks out of this module (currently into
    // the `boundary::baseline` module) but should no longer need to be `pub`
    // once the `boundary::baseline` module gets refactored into the domain
    // logic, so I think it is fine for now.
    pub input: eth::Asset,
    pub output: eth::Asset,
    pub gas: eth::Gas,
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

    fn gas(&self) -> eth::Gas {
        eth::Gas(self.segments.iter().fold(U256::zero(), |acc, segment| {
            acc.saturating_add(segment.gas.0)
        }))
    }
}
