//! "Baseline" solver implementation.
//!
//! The baseline solver is a simple solver implementation that finds the best
//! path of at most length `max_hops + 1` over a set of on-chain liquidity. It
//! **does not** try to split large orders into multiple parts and route them
//! over separate paths.

use {
    crate::{
        boundary,
        domain::{
            auction,
            eth,
            liquidity,
            order::{self, UserOrder},
            solution,
        },
    },
    ethereum_types::U256,
    std::{cmp, collections::HashSet, sync::Arc},
};

pub struct Baseline(Arc<Inner>);

/// The amount of time we aim the solver to finish before the final deadline is
/// reached.
const DEADLINE_SLACK: chrono::Duration = chrono::Duration::milliseconds(500);

pub struct Config {
    pub weth: eth::WethAddress,
    pub base_tokens: Vec<eth::TokenAddress>,
    pub max_hops: usize,
    pub max_partial_attempts: usize,
    pub solution_gas_offset: eth::SignedGas,
    pub native_token_price_estimation_amount: eth::U256,
}

struct Inner {
    weth: eth::WethAddress,

    /// Set of tokens to additionally consider as intermediary hops when
    /// path-finding. This allows paths of the kind `TOKEN1 -> WETH -> TOKEN2`
    /// to be considered.
    base_tokens: HashSet<eth::TokenAddress>,

    /// Maximum number of hops that can be considered in a trading path. A hop
    /// is an intermediary token within a trading path. For example:
    /// - A value of 0 indicates that only a direct trade is allowed: `A -> B`
    /// - A value of 1 indicates that a single intermediary token can appear
    ///   within a trading path: `A -> B -> C`
    /// - A value of 2 indicates: `A -> B -> C -> D`
    /// - etc.
    max_hops: usize,

    /// The maximum number of attempts to solve a partially fillable order.
    /// Basically we continuously halve the amount to execute until we find a
    /// valid solution or exceed this count.
    max_partial_attempts: usize,

    /// Units of gas that get added to the gas estimate for executing a
    /// computed trade route to arrive at a gas estimate for a whole settlement.
    solution_gas_offset: eth::SignedGas,

    /// The amount of the native token to use to estimate native price of a
    /// token
    native_token_price_estimation_amount: eth::U256,
}

impl Baseline {
    /// Creates a new baseline solver for the specified configuration.
    pub fn new(config: Config) -> Self {
        Self(Arc::new(Inner {
            weth: config.weth,
            base_tokens: config.base_tokens.into_iter().collect(),
            max_hops: config.max_hops,
            max_partial_attempts: config.max_partial_attempts,
            solution_gas_offset: config.solution_gas_offset,
            native_token_price_estimation_amount: config.native_token_price_estimation_amount,
        }))
    }

    /// Solves the specified auction, returning a vector of all possible
    /// solutions.
    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        // Make sure to push the CPU-heavy code to a separate thread in order to
        // not lock up the [`tokio`] runtime and cause it to slow down handling
        // the real async things. For larger settlements, this can block in the
        // 100s of ms.
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let deadline = auction
            .deadline
            .clone()
            .reduce(DEADLINE_SLACK)
            .remaining()
            .unwrap_or_default();

        let inner = self.0.clone();
        let span = tracing::Span::current();
        let background_work = async move {
            let _entered = span.enter();
            inner.solve(auction, sender);
        };

        if tokio::time::timeout(deadline, tokio::spawn(background_work))
            .await
            .is_err()
        {
            tracing::debug!("reached timeout while solving orders");
        }

        let mut solutions = vec![];
        while let Ok(solution) = receiver.try_recv() {
            solutions.push(solution);
        }
        solutions
    }
}

impl Inner {
    fn solve(
        &self,
        auction: auction::Auction,
        sender: tokio::sync::mpsc::UnboundedSender<solution::Solution>,
    ) {
        let boundary_solver =
            boundary::baseline::Solver::new(&self.weth, &self.base_tokens, &auction.liquidity);

        for (i, order) in auction.orders.into_iter().enumerate() {
            let Some(user_order) = UserOrder::new(&order) else {
                continue;
            };

            let sell_token = user_order.get().sell.token;
            let sell_token_price = match auction.tokens.reference_price(&sell_token) {
                Some(price) => price,
                None if sell_token == self.weth.0.into() => {
                    // Early return if the sell token is native token
                    auction::Price(eth::Ether(eth::U256::exp10(18)))
                }
                None => {
                    // Estimate the price of the sell token in the native token
                    let native_price_request = self.native_price_request(user_order);
                    if let Some(route) = boundary_solver.route(native_price_request, self.max_hops)
                    {
                        // how many units of buy_token are bought for one unit of sell_token
                        // (buy_amount / sell_amount).
                        let price = self.native_token_price_estimation_amount.to_f64_lossy()
                            / route.input().amount.to_f64_lossy();
                        let Some(price) = to_normalized_price(price) else {
                            continue;
                        };

                        auction::Price(eth::Ether(price))
                    } else {
                        // This is to allow quotes to be generated for tokens for which the sell
                        // token price is not available, so we default to fee=0
                        auction::Price(eth::Ether(eth::U256::MAX))
                    }
                }
            };

            let solution = self.requests_for_order(user_order).find_map(|request| {
                tracing::trace!(order =% order.uid, ?request, "finding route");

                let route = boundary_solver.route(request, self.max_hops)?;
                let interactions = route
                    .segments
                    .iter()
                    .map(|segment| {
                        solution::Interaction::Liquidity(solution::LiquidityInteraction {
                            liquidity: segment.liquidity.clone(),
                            input: segment.input,
                            output: segment.output,
                            // TODO does the baseline solver know about this optimization?
                            internalize: false,
                        })
                    })
                    .collect();

                // The baseline solver generates a path with swapping
                // for exact output token amounts. This leads to
                // potential rounding errors for buy orders, where we
                // can buy slightly more than intended. Fix this by
                // capping the output amount to the order's buy amount
                // for buy orders.
                let mut output = route.output();
                if let order::Side::Buy = order.side {
                    output.amount = cmp::min(output.amount, order.buy.amount);
                }

                let gas = route.gas() + self.solution_gas_offset;
                let fee = sell_token_price
                    .ether_value(eth::Ether(gas.0.checked_mul(auction.gas_price.0 .0)?))?
                    .into();

                Some(
                    solution::Single {
                        order: order.clone(),
                        input: route.input(),
                        output,
                        interactions,
                        gas,
                    }
                    .into_solution(fee)?
                    .with_id(solution::Id(i as u64))
                    .with_buffers_internalizations(&auction.tokens),
                )
            });
            if let Some(solution) = solution {
                if sender.send(solution).is_err() {
                    tracing::debug!("deadline hit, receiver dropped");
                    break;
                }
            }
        }
    }

    fn requests_for_order(&self, order: UserOrder) -> impl Iterator<Item = Request> {
        let order::Order {
            sell, buy, side, ..
        } = order.get().clone();

        let n = if order.get().partially_fillable {
            self.max_partial_attempts
        } else {
            1
        };

        (0..n)
            .map(move |i| {
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
            .filter(|r| !r.sell.amount.is_zero() && !r.buy.amount.is_zero())
    }

    fn native_price_request(&self, order: UserOrder) -> Request {
        let sell = eth::Asset {
            token: order.get().sell.token,
            // Note that we intentionally do not use [`eth::U256::max_value()`]
            // as an order with this would cause overflows with the smart
            // contract, so buy orders requiring excessively large sell amounts
            // would not work anyway. Instead we use `2 ** 144`, the rationale
            // being that Uniswap V2 pool reserves are 112-bit integers. Noting
            // that `256 - 112 = 144`, this means that we can use it to trade a full
            // `type(uint112).max` without overflowing a `uint256` on the smart
            // contract level. Requiring to trade more than `type(uint112).max`
            // is unlikely and would not work with Uniswap V2 anyway.
            amount: eth::U256::one() << 144,
        };

        let buy = eth::Asset {
            token: self.weth.0.into(),
            amount: self.native_token_price_estimation_amount,
        };

        Request {
            sell,
            buy,
            side: order::Side::Buy,
        }
    }
}

fn to_normalized_price(price: f64) -> Option<U256> {
    let uint_max = 2.0_f64.powi(256);

    let price_in_eth = 1e18 * price;
    if price_in_eth.is_normal() && price_in_eth >= 1. && price_in_eth < uint_max {
        Some(U256::from_f64_lossy(price_in_eth))
    } else {
        None
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
#[derive(Debug)]
pub struct Route<'a> {
    segments: Vec<Segment<'a>>,
}

/// A segment in a trading route.
#[derive(Debug)]
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
