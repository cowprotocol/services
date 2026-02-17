//! Winner Selection:
//! Implements a winner selection algorithm which picks the **set** of solutions
//! which maximize surplus while enforcing uniform **directional** clearing
//! prices. That means all orders selling the same token must get executed at
//! the same price for that token. But orders buying that same token may all be
//! settled at a different (but still uniform) price. So effectively instead of
//! allowing only 1 price for each token (uniform clearing price) each token may
//! have 2 prices (one for selling it and another for buying it).
//!
//! Fairness Guarantees:
//! A solution is only valid if it does not settle any order at a worse uniform
//! directional clearing price than the best solution which only contains this
//! uniform directional clearing price. In other words an order may only be
//! batched with other orders if each order gets a better deal than executing
//! it individually.
//! Because these guarantees rely heavily on all relevant scores of each
//! solution being computed, we'll discard solutions where that computation
//! fails.
//!
//! Reference Score:
//! Each solver S with a winning solution gets one reference score. The
//! reference score is the total score of all winning solutions if the solver S
//! had not participated in the competition.
//! That is effectively a measurement of how much better each order got executed
//! because solver S participated in the competition.

use {
    crate::domain::{
        self,
        auction::order,
        competition::{Bid, RankType, Ranked, Score, Solution, TradedOrder, Unscored},
        eth::{self, WrappedNativeToken},
        fee,
    },
    ::winner_selection::state::{HasState, RankedItem, ScoredItem, UnscoredItem},
    std::collections::HashMap,
    winner_selection::{self as winsel},
};

pub struct Arbitrator(winsel::Arbitrator);

/// Implements auction arbitration in 3 phases:
/// 1. filter unfair solutions
/// 2. mark winners
/// 3. compute reference scores
///
/// The functions assume the `Arbitrator` is the only one
/// changing the ordering or the `bids`.
impl Arbitrator {
    pub fn new(max_winners: usize, wrapped_native_token: WrappedNativeToken) -> Self {
        let token: eth::TokenAddress = wrapped_native_token.into();
        Self(winsel::Arbitrator {
            max_winners,
            weth: token.0,
        })
    }

    /// Runs the entire auction mechanism on the passed in solutions.
    pub fn arbitrate(&self, bids: Vec<Bid<Unscored>>, auction: &domain::Auction) -> Ranking {
        let context = auction.into();
        let mut bid_by_key = HashMap::with_capacity(bids.len());
        let mut solutions = Vec::with_capacity(bids.len());

        for bid in bids {
            let key = SolutionKey::from(bid.solution());
            let solution = bid.solution().into();
            bid_by_key.insert(key, bid);
            solutions.push(solution);
        }

        let ws_ranking = self.0.arbitrate(solutions, &context);

        // Compute reference scores while we still have ws_ranking
        let reference_scores: HashMap<eth::Address, Score> = self
            .0
            .compute_reference_scores(&ws_ranking)
            .into_iter()
            .map(|(solver, score)| (solver, Score(eth::Ether(score))))
            .collect();

        let mut filtered_out = Vec::with_capacity(ws_ranking.filtered_out.len());
        for ws_solution in ws_ranking.filtered_out {
            let key = SolutionKey::from(&ws_solution);
            let bid = bid_by_key
                .remove(&key)
                .expect("every ranked solution has a matching bid");
            let score = ws_solution.score();
            filtered_out.push(
                bid.with_score(Score(eth::Ether(score)))
                    .with_rank(RankType::FilteredOut),
            );
        }

        let mut ranked = Vec::with_capacity(ws_ranking.ranked.len());
        for ranked_solution in ws_ranking.ranked {
            let key = SolutionKey::from(&ranked_solution);
            let bid = bid_by_key
                .remove(&key)
                .expect("every ranked solution has a matching bid");
            let score = ranked_solution.score();
            ranked.push(
                bid.with_score(Score(eth::Ether(score)))
                    .with_rank(ranked_solution.state().rank_type),
            );
        }

        Ranking {
            filtered_out,
            ranked,
            reference_scores,
        }
    }
}

impl From<&domain::Auction> for winsel::AuctionContext {
    fn from(auction: &domain::Auction) -> Self {
        Self {
            fee_policies: auction
                .orders
                .iter()
                .map(|order| {
                    let uid = winsel::OrderUid(order.uid.0);
                    let policies = order
                        .protocol_fees
                        .iter()
                        .copied()
                        .map(winsel::primitives::FeePolicy::from)
                        .collect();
                    (uid, policies)
                })
                .collect(),
            surplus_capturing_jit_order_owners: auction
                .surplus_capturing_jit_order_owners
                .iter()
                .copied()
                .collect(),
            native_prices: auction
                .prices
                .iter()
                .map(|(token, price)| (token.0, price.get().0))
                .collect(),
        }
    }
}

impl From<&Solution> for winsel::Solution<winsel::Unscored> {
    fn from(solution: &Solution) -> Self {
        Self::new(
            solution.id(),
            solution.solver(),
            solution
                .orders()
                .iter()
                .map(|(uid, order)| to_winsel_order(*uid, order))
                .collect(),
            solution
                .prices()
                .iter()
                .map(|(token, price)| (token.0, price.get().0))
                .collect(),
        )
    }
}

fn to_winsel_order(uid: domain::OrderUid, order: &TradedOrder) -> winsel::Order {
    winsel::Order {
        uid: winsel::OrderUid(uid.0),
        sell_token: order.sell.token.0,
        buy_token: order.buy.token.0,
        sell_amount: order.sell.amount.0,
        buy_amount: order.buy.amount.0,
        executed_sell: order.executed_sell.0,
        executed_buy: order.executed_buy.0,
        side: match order.side {
            order::Side::Buy => winsel::Side::Buy,
            order::Side::Sell => winsel::Side::Sell,
        },
    }
}

impl From<fee::Policy> for winsel::primitives::FeePolicy {
    fn from(policy: fee::Policy) -> Self {
        match policy {
            fee::Policy::Surplus {
                factor,
                max_volume_factor,
            } => Self::Surplus {
                factor: factor.get(),
                max_volume_factor: max_volume_factor.get(),
            },
            fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self::PriceImprovement {
                factor: factor.get(),
                max_volume_factor: max_volume_factor.get(),
                quote: winsel::primitives::Quote {
                    sell_amount: quote.sell_amount,
                    buy_amount: quote.buy_amount,
                    fee: quote.fee,
                    solver: quote.solver,
                },
            },
            fee::Policy::Volume { factor } => Self::Volume {
                factor: factor.get(),
            },
        }
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct SolutionKey {
    solver: eth::Address,
    solution_id: u64,
}

impl From<&Solution> for SolutionKey {
    fn from(solution: &Solution) -> Self {
        Self {
            solver: solution.solver(),
            solution_id: solution.id(),
        }
    }
}

impl<S> From<&winsel::Solution<S>> for SolutionKey {
    fn from(solution: &winsel::Solution<S>) -> Self {
        Self {
            solver: solution.solver(),
            solution_id: solution.id(),
        }
    }
}

pub struct Ranking {
    /// Solutions that were discarded because they were malformed
    /// in some way or deemed unfair by the selection mechanism.
    filtered_out: Vec<Bid<Ranked>>,
    /// Final ranking of the solutions that passed the fairness
    /// check. Winners come before non-winners and higher total
    /// scores come before lower scores.
    ranked: Vec<Bid<Ranked>>,
    /// Reference scores for each winning solver, used to compute rewards.
    reference_scores: HashMap<eth::Address, Score>,
}

impl Ranking {
    /// All solutions including the ones that got filtered out.
    pub fn all(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter().chain(&self.filtered_out)
    }

    /// Enumerates all solutions. The index is used as solution UID.
    pub fn enumerated(&self) -> impl Iterator<Item = (usize, &Bid<Ranked>)> {
        self.all().enumerate()
    }

    /// All solutions that won the right to get executed.
    pub fn winners(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter().filter(|b| b.is_winner())
    }

    /// All solutions that were not filtered out but also did not win.
    pub fn non_winners(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter().filter(|b| !b.is_winner())
    }

    /// Reference scores for each winning solver, used to compute rewards.
    pub fn reference_scores(&self) -> &HashMap<eth::Address, Score> {
        &self.reference_scores
    }

    /// All solutions that passed the filtering step.
    pub fn ranked(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter()
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{
            config::solver::Account,
            domain::{
                Auction,
                Order,
                OrderUid,
                auction::{
                    Price,
                    order::{self, AppDataHash},
                },
                competition::{Bid, Solution, TradedOrder, Unscored},
                eth::{self, TokenAddress},
            },
            infra::Driver,
        },
        alloy::primitives::{Address, U160, U256, address},
        hex_literal::hex,
        number::serialization::HexOrDecimalU256,
        serde::Deserialize,
        serde_json::json,
        serde_with::serde_as,
        std::{
            collections::HashMap,
            hash::{DefaultHasher, Hash, Hasher},
        },
        winner_selection::state::RankedItem,
    };

    const DEFAULT_TOKEN_PRICE: u128 = 1_000;

    #[tokio::test]
    // Only one bid submitted results in one winner with reference score = 0
    async fn single_bid() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "sell",
                        "sell_token": "Token C",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    }
                }
            },
            "solutions": {
                // score = 200
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                }
            },
            "expected_fair_solutions": ["Solution 1"],
            "expected_winners": ["Solution 1"],
            "expected_reference_scores": {
                "Solver 1": "0",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    // Two compatible batches are both selected as winners
    async fn compatible_bids() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "sell",
                        "sell_token": "Token C",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    },
                    "Order 3": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token C",
                        "buy_amount": amount(1_000)
                    }
                }
            },
            "solutions": {
                // score = 200
                "Best batch": {
                    "solver": "Best batch solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                },
                // score = 100
                "Compatible batch": {
                    "solver": "Compatible batch solver",
                    "trades": {
                        "Order 3": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                }
            },
            "expected_fair_solutions": ["Best batch", "Compatible batch"],
            "expected_winners": ["Best batch", "Compatible batch"],
            "expected_reference_scores": {
                "Best batch solver": "100",
                "Compatible batch solver": "200",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    // Two compatible batches are both selected as winners, but this time the orders
    // are "buy" orders
    async fn buy_orders() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "buy",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "buy",
                        "sell_token": "Token C",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    },
                    "Order 3": {
                        "side": "buy",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token C",
                        "buy_amount": amount(1_000)
                    }
                }
            },
            "solutions": {
                // score = 200
                "Best batch": {
                    "solver": "Best batch solver",
                    "trades": {
                        // less sell tokens are used to get the expected buy tokens, that is the surplus
                        "Order 1": {
                            "sell_amount": amount(900),
                            "buy_amount": amount(1_000)
                        },
                        "Order 2": {
                            "sell_amount": amount(900),
                            "buy_amount": amount(1_000)
                        }
                    }
                },
                // score = 100
                "Compatible batch": {
                    "solver": "Compatible batch solver",
                    "trades": {
                        "Order 3": {
                            "sell_amount": amount(900),
                            "buy_amount": amount(1_000)
                        }
                    },
                }
            },
            "expected_fair_solutions": ["Best batch", "Compatible batch"],
            "expected_winners": ["Best batch", "Compatible batch"],
            "expected_reference_scores": {
                "Best batch solver": "100",
                "Compatible batch solver": "200",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    // Multiple compatible bids by a single solver are aggregated
    async fn multiple_solution_for_solver() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "sell",
                        "sell_token": "Token C",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    },
                    "Order 3": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    }
                }
            },
            "solutions": {
                // score = 200
                "Best batch": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    },
                },
                // score = 100
                "Compatible batch": {
                    "solver": "Solver 1", // same solver
                    "trades": {
                        "Order 3": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    },
                }
            },
            "expected_fair_solutions": ["Best batch", "Compatible batch"],
            "expected_winners": ["Best batch", "Compatible batch"],
            "expected_reference_scores": {
                "Solver 1": "0",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    // Incompatible bid does not win but increases the reference score of the winner
    async fn incompatible_bids() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "sell",
                        "sell_token": "Token C",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    },
                }
            },
            "solutions": {
                // score = 200
                "Best batch": {
                    "solver": "Best batch solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                },
                // score = 100
                "Compatible batch": {
                    "solver": "Compatible batch solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                }
            },
            "expected_fair_solutions": ["Best batch", "Compatible batch"],
            "expected_winners": ["Best batch"],
            "expected_reference_scores": {
                "Best batch solver": "100",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    // Unfair batch is filtered
    async fn fairness_filtering() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "sell",
                        "sell_token": "Token C",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    },
                }
            },
            "solutions": {
                // score = 200
                "Unfair batch": {
                    "solver": "Unfair batch solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                },
                // score = 150
                "Filtering batch": {
                    "solver": "Filtering batch solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_150)
                        }
                    }
                }
            },
            "expected_fair_solutions": ["Filtering batch"],
            "expected_winners": ["Filtering batch"],
            "expected_reference_scores": {
                "Filtering batch solver": "0",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    // Multiple trades on the same (directed) token pair are aggregated for
    // filtering
    async fn aggregation_on_token_pair() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    }
                }
            },
            "solutions": {
                // score = 200
                "Batch with aggregation": {
                    "solver": "Batch with aggregation solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                },
                // score = 150
                "Incompatible batch": {
                    "solver": "Incompatible batch solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_150)
                        }
                    }
                }
            },
            "expected_fair_solutions": ["Batch with aggregation", "Incompatible batch"],
            "expected_winners": ["Batch with aggregation"],
            "expected_reference_scores": {
                "Batch with aggregation solver": "150",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    // Reference winners can generate more surplus than winners
    async fn reference_better_than_winners() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)],
                ["Token E", address(4)],
                ["Token F", address(5)]
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token B",
                        "buy_amount": amount(1_000)
                    },
                    "Order 2": {
                        "side": "sell",
                        "sell_token": "Token C",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token D",
                        "buy_amount": amount(1_000)
                    },
                    "Order 3": {
                        "side": "sell",
                        "sell_token": "Token E",
                        "sell_amount": amount(1_000),
                        "buy_token": "Token F",
                        "buy_amount": amount(1_000)
                    }
                }
            },
            "solutions": {
                // score = 300
                "Best batch": {
                    "solver": "Best batch solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        },
                        "Order 3": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                },
                // score = 280
                "Incompatible batch 1": {
                    "solver": "Incompatible batch 1 solver",
                    "trades": {
                        "Order 1": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_140)
                        },
                        "Order 2": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_140)
                        }
                    }
                },
                // score = 100
                "Incompatible batch 2": {
                    "solver": "Incompatible batch 2 solver",
                    "trades": {
                        "Order 3": {
                            "sell_amount": amount(1_000),
                            "buy_amount": amount(1_100)
                        }
                    }
                }
            },
            "expected_fair_solutions": ["Best batch", "Incompatible batch 1",  "Incompatible batch 2"],
            "expected_winners": ["Best batch"],
            "expected_reference_scores": {
                "Best batch solver": "380",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[tokio::test]
    async fn staging_mainnet_auction_12825008() {
        // https://solver-instances.s3.eu-central-1.amazonaws.com/staging/mainnet/autopilot/12825008.json
        // The example is an auction with one order and two competing bids for it, one
        // having a better score than the other

        let case = json!({
            "tokens": [
                // corresponding to 0x67466be17df832165f8c80a5a120ccc652bd7e69
                ["Token A", address(0)],
                // corresponding to 0xdac17f958d2ee523a2206206994597c13d831ec7
                ["Token B", address(1)],
            ],
            "auction": {
                "orders": {
                    "Order 1": {
                        "side": "sell",
                        "sell_token": "Token A",
                        "sell_amount": "32375066190000000000000000",
                        "buy_token": "Token B",
                        "buy_amount": "2161512119"
                    }
                },
                "prices": {
                    "Token A": "32429355240",
                    "Token B": "480793239987749750742974464"
                }
            },
            "solutions": {
                // solution 1 (baseline, the winner)
                // score = 21813202259686016
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": {
                            "sell_amount": "32375066190000000000000000",
                            "buy_amount": "2206881314"
                        }
                    }
                },
                // solution 2 (zeroex)
                // score = 21037471695353421
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 1": {
                            "sell_amount": "32375066190000000000000000",
                            "buy_amount": "2205267875"
                        }
                    }
                }
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2"],
            "expected_winners": ["Solution 1"],
            "expected_reference_scores": {
                "Solver 1": "21037471695353421",
            },
        });
        TestCase::from_json(case).validate().await;
    }

    #[serde_as]
    #[derive(Deserialize, Debug)]
    struct TestCase {
        pub tokens: Vec<(String, Address)>,
        pub auction: TestAuction,
        pub solutions: HashMap<String, TestSolution>,
        pub expected_fair_solutions: Vec<String>,
        pub expected_winners: Vec<String>,
        #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
        pub expected_reference_scores: HashMap<String, eth::U256>,
    }

    impl TestCase {
        pub fn from_json(value: serde_json::Value) -> Self {
            serde_json::from_value(value).unwrap()
        }

        pub async fn validate(&self) {
            let arbitrator = create_test_arbitrator();

            // map (token id -> token address) for later reference during the test
            let token_map: HashMap<String, TokenAddress> = self
                .tokens
                .iter()
                .cloned()
                .map(|(id, address)| (id, address.into()))
                .collect();

            // map (order id -> order) for later reference during the test
            let order_map: HashMap<String, Order> = self
                .auction
                .orders
                .iter()
                .map(
                    |(
                        order_id,
                        TestOrder {
                            side,
                            sell_token,
                            sell_amount,
                            buy_token,
                            buy_amount,
                        },
                    )| {
                        let order_uid = hash(order_id);
                        let sell_token = token_map.get(sell_token).unwrap();
                        let buy_token = token_map.get(buy_token).unwrap();
                        let order = create_order(
                            order_uid,
                            *sell_token,
                            *sell_amount,
                            *buy_token,
                            *buy_amount,
                            *side,
                        );
                        (order_id.clone(), order)
                    },
                )
                .collect();

            let orders = order_map.values().cloned().collect();
            let prices = self.auction.prices.as_ref().map(|prices| {
                prices
                    .iter()
                    .map(|(token_id, price)| {
                        let token_address = *token_map.get(token_id).unwrap();
                        let price = create_price(*price);
                        (token_address, price)
                    })
                    .collect()
            });

            let auction = create_auction(orders, prices);

            // map (solver id -> solver address) for later reference during the test
            let mut solver_map = HashMap::new();

            // map (solution id -> bid) for later reference during the test
            let mut solution_map = HashMap::new();
            for (solution_id, solution) in &self.solutions {
                // generate solver address deterministically from the id
                let solver_uid = hash(&solution.solver);
                let solver_address = address(solver_uid);
                solver_map.insert(solution.solver.clone(), solver_address);

                let trades = solution
                    .trades
                    .iter()
                    .map(|(order_id, trade)| {
                        let order = order_map.get(order_id).unwrap();
                        let sell_token_amount = trade.sell_amount;
                        let buy_token_amount = trade.buy_amount;
                        let trade = create_trade(order, sell_token_amount, buy_token_amount);
                        (order.uid, trade)
                    })
                    .collect();

                let solution_uid = hash(solution_id);
                solution_map.insert(
                    solution_id,
                    create_bid(solution_uid, solver_address, trades, None).await,
                );
            }

            // filter solutions
            let bids = solution_map.values().cloned().collect();
            let ranking = arbitrator.arbitrate(bids, &auction);
            assert_eq!(ranking.ranked.len(), self.expected_fair_solutions.len());
            for solution_id in &self.expected_fair_solutions {
                let solution_uid = solution_map.get(&solution_id).unwrap().solution().id;
                assert!(
                    ranking
                        .ranked
                        .iter()
                        .any(|s| s.solution().id == solution_uid)
                );
            }

            let winners = filter_winners(&ranking.ranked);
            assert!(winners.iter().is_sorted_by_key(|a| (
                // winners before non-winners
                std::cmp::Reverse(a.is_winner()),
                // high score before low score
                std::cmp::Reverse(a.score())
            )));
            assert_eq!(winners.len(), self.expected_winners.len());
            for (actual, expected) in winners.iter().zip(&self.expected_winners) {
                let solution_uid = solution_map.get(&expected).unwrap().solution().id;
                assert_eq!(actual.solution().id, solution_uid);
            }

            // compute reference score
            let reference_scores = ranking.reference_scores();
            assert_eq!(reference_scores.len(), self.expected_reference_scores.len());
            for (solver_id, expected_score) in &self.expected_reference_scores {
                let solver_address = solver_map.get(solver_id).unwrap();
                let score = reference_scores.get(solver_address).unwrap();
                assert_eq!(score.0, eth::Ether(*expected_score))
            }
        }
    }

    #[serde_as]
    #[derive(Deserialize, Debug)]
    struct TestAuction {
        pub orders: HashMap<String, TestOrder>,
        #[serde(default)]
        #[serde_as(as = "Option<HashMap<_, HexOrDecimalU256>>")]
        pub prices: Option<HashMap<String, eth::U256>>,
    }

    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    struct TestOrder {
        #[serde(deserialize_with = "deserialize_side")]
        pub side: order::Side,
        pub sell_token: String,
        #[serde_as(as = "HexOrDecimalU256")]
        pub sell_amount: eth::U256,
        pub buy_token: String,
        #[serde_as(as = "HexOrDecimalU256")]
        pub buy_amount: eth::U256,
    }

    #[derive(Deserialize, Debug)]
    struct TestSolution {
        pub solver: String,
        pub trades: HashMap<String, TestTrade>,
    }

    #[serde_as]
    #[derive(Deserialize, Debug)]
    struct TestTrade {
        #[serde_as(as = "HexOrDecimalU256")]
        pub sell_amount: eth::U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub buy_amount: eth::U256,
    }

    fn create_test_arbitrator() -> super::Arbitrator {
        super::Arbitrator::new(
            10,
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").into(),
        )
    }

    fn address(id: u64) -> Address {
        Address::from(U160::from(id))
    }

    fn create_order(
        uid: u64,
        sell_token: TokenAddress,
        sell_amount: eth::U256,
        buy_token: TokenAddress,
        buy_amount: eth::U256,
        side: order::Side,
    ) -> Order {
        Order {
            uid: create_order_uid(uid),
            sell: eth::Asset {
                amount: sell_amount.into(),
                token: sell_token,
            },
            buy: eth::Asset {
                amount: buy_amount.into(),
                token: buy_token,
            },
            protocol_fees: vec![],
            side,
            receiver: None,
            owner: Default::default(),
            partially_fillable: false,
            executed: eth::U256::ZERO.into(),
            pre_interactions: vec![],
            post_interactions: vec![],
            sell_token_balance: order::SellTokenSource::Erc20,
            buy_token_balance: order::BuyTokenDestination::Erc20,
            app_data: AppDataHash(hex!(
                "6000000000000000000000000000000000000000000000000000000000000007"
            )),
            created: Default::default(),
            valid_to: Default::default(),
            signature: order::Signature::PreSign,
            quote: None,
        }
    }

    // Deterministically creates an OrderUid from a u64.
    fn create_order_uid(uid: u64) -> OrderUid {
        let mut encoded_uid = [0u8; 56];
        let uid_bytes = uid.to_le_bytes();
        encoded_uid[..uid_bytes.len()].copy_from_slice(&uid_bytes);
        OrderUid(encoded_uid)
    }

    fn create_price(value: eth::U256) -> Price {
        Price::try_new(eth::Ether(value)).unwrap()
    }

    fn create_auction(
        orders: Vec<Order>,
        prices: Option<HashMap<eth::TokenAddress, Price>>,
    ) -> Auction {
        // Initialize the prices of the tokens if they are not specified
        let prices = prices.unwrap_or({
            let default_price = create_price(U256::from(DEFAULT_TOKEN_PRICE));
            let mut res = HashMap::new();
            for order in &orders {
                res.insert(order.buy.token, default_price);
                res.insert(order.sell.token, default_price);
            }
            res
        });

        Auction {
            id: 0,
            block: 0,
            orders,
            prices,
            surplus_capturing_jit_order_owners: vec![],
        }
    }

    fn create_trade(
        order: &Order,
        executed_sell: eth::U256,
        executed_buy: eth::U256,
    ) -> TradedOrder {
        TradedOrder {
            side: order.side,
            sell: order.sell,
            buy: order.buy,
            executed_sell: executed_sell.into(),
            executed_buy: executed_buy.into(),
        }
    }

    async fn create_bid(
        solution_id: u64,
        solver_address: Address,
        trades: Vec<(OrderUid, TradedOrder)>,
        prices: Option<HashMap<TokenAddress, Price>>,
    ) -> Bid<Unscored> {
        // The prices of the tokens do not affect the result but they keys must exist
        // for every token of every trade
        let prices = prices.unwrap_or({
            let mut res = HashMap::new();
            for (_, trade) in &trades {
                res.insert(trade.buy.token, create_price(eth::U256::ONE));
                res.insert(trade.sell.token, create_price(eth::U256::ONE));
            }
            res
        });

        let trade_order_map: HashMap<OrderUid, TradedOrder> = trades.into_iter().collect();

        let solution = Solution::new(solution_id, solver_address, trade_order_map, prices);

        let driver = Driver::try_new(
            url::Url::parse("http://localhost").unwrap(),
            solver_address.to_string(),
            Account::Address(solver_address),
        )
        .await
        .unwrap();

        Bid::new(solution, std::sync::Arc::new(driver))
    }

    fn amount(value: u128) -> String {
        // adding decimal units to avoid the math rounding it down to 0
        to_e15(value).to_string()
    }

    fn to_e15(value: u128) -> u128 {
        value * 10u128.pow(15)
    }

    fn filter_winners(bids: &[Bid]) -> Vec<&Bid> {
        bids.iter().filter(|b| b.is_winner()).collect()
    }

    // Used to generate deterministic identifiers (e.g., UIDs, addresses) from
    // string descriptions.
    fn hash(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    // Needed to automatically deserialize order::Side in JSON test cases
    fn deserialize_side<'de, D>(deserializer: D) -> Result<order::Side, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "buy" => Ok(order::Side::Buy),
            "sell" => Ok(order::Side::Sell),
            _ => Err(serde::de::Error::custom(format!("Invalid side: {s}"))),
        }
    }
}
