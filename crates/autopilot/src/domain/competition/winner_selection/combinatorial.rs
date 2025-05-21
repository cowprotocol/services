//! Winner Selection:
//! Implements a winner selction algorithm which picks the **set** of solutions
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
    super::Arbitrator,
    crate::domain::{
        self,
        OrderUid,
        auction::{
            Prices,
            order::{self, TargetAmount},
        },
        competition::{Participant, Score, Solution, Unranked},
        eth::{self, WrappedNativeToken},
        fee,
        settlement::{
            math,
            transaction::{self, ClearingPrices},
        },
    },
    anyhow::{Context, Result},
    itertools::Itertools,
    std::{
        collections::{HashMap, HashSet},
        ops::Add,
    },
};

impl Arbitrator for Config {
    fn filter_unfair_solutions(
        &self,
        mut participants: Vec<Participant<Unranked>>,
        auction: &domain::Auction,
    ) -> Vec<Participant<Unranked>> {
        // Discard all solutions where we can't compute the aggregate scores
        // accurately because the fairness guarantees heavily rely on them.
        let scores_by_solution = compute_scores_by_solution(&mut participants, auction);
        participants.sort_unstable_by_key(|participant| {
            std::cmp::Reverse(participant.solution().score().get().0)
        });
        let baseline_scores = compute_baseline_scores(&scores_by_solution);
        participants.retain(|p| {
            let aggregated_scores = scores_by_solution
                .get(&SolutionKey {
                    driver: p.driver().submission_address,
                    solution_id: p.solution().id(),
                })
                .expect("every remaining participant has an entry");
            // only keep solutions where each order execution is at least as good as
            // the baseline solution (or when there is only one baseline solution)
            aggregated_scores.len() == 1
                || aggregated_scores.iter().all(|(pair, score)| {
                    baseline_scores
                        .get(pair)
                        .is_none_or(|baseline| score >= baseline)
                })
        });
        participants
    }

    fn mark_winners(&self, participants: Vec<Participant<Unranked>>) -> Vec<Participant> {
        let winner_indexes = self.pick_winners(participants.iter().map(|p| p.solution()));
        participants
            .into_iter()
            .enumerate()
            .map(|(index, participant)| participant.rank(winner_indexes.contains(&index)))
            .collect()
    }

    fn compute_reference_scores(
        &self,
        participants: &[Participant],
    ) -> HashMap<eth::Address, Score> {
        let mut reference_scores = HashMap::default();

        for participant in participants {
            let solver = participant.driver().submission_address;
            if reference_scores.len() >= self.max_winners {
                // all winners have been processed
                return reference_scores;
            }
            if reference_scores.contains_key(&solver) {
                // we already computed this solver's reference score
                continue;
            }

            let solutions_without_solver = participants
                .iter()
                .filter(|p| p.driver().submission_address != solver)
                .map(|p| p.solution());
            let winner_indices = self.pick_winners(solutions_without_solver.clone());

            let score = solutions_without_solver
                .enumerate()
                .filter(|(index, _)| winner_indices.contains(index))
                .filter_map(|(_, solution)| solution.computed_score)
                .reduce(Score::add)
                .unwrap_or_default();
            reference_scores.insert(solver, score);
        }

        reference_scores
    }
}

impl Config {
    /// Returns indices of winning solutions.
    /// Assumes that `solutions` is sorted by score descendingly.
    /// This logic was moved into a helper function to avoid a ton of `.clone()`
    /// operations in `compute_reference_scores()`.
    fn pick_winners<'a>(&self, solutions: impl Iterator<Item = &'a Solution>) -> HashSet<usize> {
        // Winners are selected one by one, starting from the best solution,
        // until `max_winners` are selected. A solution can only
        // win if none of the (sell_token, buy_token) pairs of the executed
        // orders have been covered by any previously selected winning solution.
        // In other words this enforces a uniform **directional** clearing price.
        let mut already_swapped_tokens_pairs = HashSet::new();
        let mut winners = HashSet::default();
        for (index, solution) in solutions.enumerate() {
            if winners.len() >= self.max_winners {
                return winners;
            }

            let swapped_token_pairs = solution
                .orders()
                .values()
                .map(|order| DirectedTokenPair {
                    sell: order.sell.token.as_erc20(self.weth),
                    buy: order.buy.token.as_erc20(self.weth),
                })
                .collect::<HashSet<_>>();

            if swapped_token_pairs.is_disjoint(&already_swapped_tokens_pairs) {
                winners.insert(index);
                already_swapped_tokens_pairs.extend(swapped_token_pairs);
            }
        }
        winners
    }
}

/// Let's call a solution that only trades 1 directed token pair a baseline
/// solution. Returns the best baseline solution (highest score) for
/// each token pair if one exists.
fn compute_baseline_scores(scores_by_solution: &ScoresBySolution) -> ScoreByDirection {
    let mut baseline_directional_scores = ScoreByDirection::default();
    for scores in scores_by_solution.values() {
        let Ok((token_pair, score)) = scores.iter().exactly_one() else {
            // base solutions must contain exactly 1 directed token pair
            continue;
        };
        let current_best_score = baseline_directional_scores
            .entry(token_pair.clone())
            .or_default();
        if score > current_best_score {
            *current_best_score = *score;
        }
    }
    baseline_directional_scores
}

/// Computes the `DirectionalScores` for all solutions and discards
/// solutions as invalid whenever that computation is not possible.
/// Solutions get discarded because fairness guarantees heavily
/// depend on these scores being accurate.
fn compute_scores_by_solution(
    participants: &mut Vec<Participant<Unranked>>,
    auction: &domain::Auction,
) -> ScoresBySolution {
    let auction = Auction::from(auction);
    let mut scores = HashMap::default();

    participants.retain_mut(|p| match score_by_token_pair(p.solution(), &auction) {
        Ok(score) => {
            let total_score = score
                .values()
                .fold(Default::default(), |acc, score| acc + *score);
            scores.insert(
                SolutionKey {
                    driver: p.driver().submission_address,
                    solution_id: p.solution().id,
                },
                score,
            );
            p.set_computed_score(total_score);
            true
        }
        Err(err) => {
            tracing::warn!(
                driver = p.driver().name,
                ?err,
                solution = ?p.solution(),
                "discarding solution where scores could not be computed"
            );
            false
        }
    });

    scores
}

/// Returns the total scores for each directed token pair of the solution.
/// E.g. if a solution contains 3 orders like:
///     sell A for B with a score of 10
///     sell A for B with a score of 5
///     sell B for C with a score of 5
/// it will return a map like:
///     (A, B) => 15
///     (B, C) => 5
fn score_by_token_pair(solution: &Solution, auction: &Auction) -> Result<ScoreByDirection> {
    let mut scores = HashMap::default();
    for (uid, trade) in solution.orders() {
        if !auction.contributes_to_score(uid) {
            continue;
        }

        let uniform_sell_price = solution
            .prices()
            .get(&trade.sell.token)
            .context("no uniform clearing price for sell token")?;
        let uniform_buy_price = solution
            .prices()
            .get(&trade.buy.token)
            .context("no uniform clearing price for buy token")?;

        let trade = math::Trade {
            uid: *uid,
            sell: trade.sell,
            buy: trade.buy,
            side: trade.side,
            executed: match trade.side {
                order::Side::Buy => TargetAmount(trade.executed_buy.into()),
                order::Side::Sell => TargetAmount(trade.executed_sell.into()),
            },
            prices: transaction::Prices {
                // clearing prices are denominated in the same underlying
                // unit so we assign sell to sell and buy to buy
                uniform: ClearingPrices {
                    sell: uniform_sell_price.get().into(),
                    buy: uniform_buy_price.get().into(),
                },
                // for custom clearing prices we only need to know how
                // much the traded tokens are worth relative to each
                // other so we can simply use the swapped executed
                // amounts here
                custom: ClearingPrices {
                    sell: trade.executed_buy.into(),
                    buy: trade.executed_sell.into(),
                },
            },
        };
        let score = trade
            .score(&auction.fee_policies, auction.native_prices)
            .context("failed to compute score")?;

        let token_pair = DirectedTokenPair {
            sell: trade.sell.token,
            buy: trade.buy.token,
        };

        *scores.entry(token_pair).or_default() += Score(score);
    }
    Ok(scores)
}

pub struct Config {
    pub max_winners: usize,
    pub weth: WrappedNativeToken,
}

/// Relevant data from `domain::Auction` but with data structures
/// optimized for the winner selection logic.
/// Avoids clones whenever possible.
struct Auction<'a> {
    /// Fee policies for **all** orders that were in the original auction.
    fee_policies: HashMap<OrderUid, &'a Vec<fee::Policy>>,
    surplus_capturing_jit_order_owners: HashSet<eth::Address>,
    native_prices: &'a Prices,
}

impl Auction<'_> {
    /// Returns whether an order is allowed to capture surplus and
    /// therefore contributes to the total score of a solution.
    fn contributes_to_score(&self, uid: &OrderUid) -> bool {
        self.fee_policies.contains_key(uid)
            || self
                .surplus_capturing_jit_order_owners
                .contains(&uid.owner())
    }
}

impl<'a> From<&'a domain::Auction> for Auction<'a> {
    fn from(original: &'a domain::Auction) -> Self {
        Self {
            fee_policies: original
                .orders
                .iter()
                .map(|o| (o.uid, &o.protocol_fees))
                .collect(),
            native_prices: &original.prices,
            surplus_capturing_jit_order_owners: original
                .surplus_capturing_jit_order_owners
                .iter()
                .cloned()
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct DirectedTokenPair {
    sell: eth::TokenAddress,
    buy: eth::TokenAddress,
}

/// Key to uniquely identify every solution.
#[derive(PartialEq, Eq, std::hash::Hash)]
struct SolutionKey {
    driver: eth::Address,
    solution_id: u64,
}

/// Scores of all trades in a solution aggregated by the directional
/// token pair. E.g. all trades (WETH -> USDC) are aggregated into
/// one value and all trades (USDC -> WETH) into another.
type ScoreByDirection = HashMap<DirectedTokenPair, Score>;

/// Mapping from solution to `DirectionalScores` for all solutions
/// of the auction.
type ScoresBySolution = HashMap<SolutionKey, ScoreByDirection>;

#[cfg(test)]
mod tests {
    use {
        crate::{
            domain::{
                auction::{
                    order::{self, AppDataHash}, Price
                }, competition::{
                    winner_selection::Arbitrator, Participant, Score, Solution, TradedOrder, Unranked
                }, eth::{self, TokenAddress}, Auction, Order, OrderUid
            },
            infra::Driver,
        },
        ethcontract::H160,
        hex_literal::hex,
        number::serialization::HexOrDecimalU256,
        serde::Deserialize,
        serde_json::json,
        serde_with::serde_as,
        std::{
            collections::{HashMap, HashSet}, hash::{DefaultHasher, Hash, Hasher}, panic::{catch_unwind, AssertUnwindSafe}, str::FromStr
        },
    };

    const DEFAULT_TOKEN_PRICE: u128 = 1_000;

    #[test]
    // Only one bid submitted results in one winner with reference score = 0
    fn single_bid() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["sell", "Token C", amount(1_000), "Token D", amount(1_000)]
                }
            },
            "solutions": {
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)]
                    },
                    "score": score(200),
                }
            },
            "expected_fair_solutions": ["Solution 1"],
            "expected_winners": ["Solution 1"],
            "expected_reference_scores": {
                "Solver 1": "0",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    // Two compatible batches are both selected as winners
    fn compatible_bids() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["sell", "Token C", amount(1_000), "Token D", amount(1_000)],
                    "Order 3": ["sell", "Token A", amount(1_000), "Token C", amount(1_000)]
                }
            },
            "solutions": {
                // best batch
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)],
                        "Order 2": [amount(1_000), amount(1_100)]
                    },
                    "score": score(200),
                },
                // compatible batch
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 3": [amount(1_000), amount(1_100)],
                    },
                    "score": score(100),
                }
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2"],
            "expected_winners": ["Solution 1", "Solution 2"],
            "expected_reference_scores": {
                "Solver 1": "100",
                "Solver 2": "200",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    // Two compatible batches are both selected as winners, but this time the orders
    // are "buy" orders
    fn buy_orders() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": ["buy", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["buy", "Token C", amount(1_000), "Token D", amount(1_000)],
                    "Order 3": ["buy", "Token A", amount(1_000), "Token C", amount(1_000)]
                }
            },
            "solutions": {
                // best batch
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        // less sell tokens are used to get the expected buy tokens
                        "Order 1": [amount(900), amount(1_000)],
                        "Order 2": [amount(900), amount(1_000)]
                    },
                    "score": score(200),
                },
                // compatible batch
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 3": [amount(900), amount(1_000)],
                    },
                    "score": score(100),
                }
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2"],
            "expected_winners": ["Solution 1", "Solution 2"],
            "expected_reference_scores": {
                "Solver 1": "100",
                "Solver 2": "200",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    // Multiple compatible bids by a single solver are aggregated
    fn multiple_solution_for_solver() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["sell", "Token C", amount(1_000), "Token D", amount(1_000)],
                    "Order 3": ["sell", "Token A", amount(1_000), "Token D", amount(1_000)]
                }
            },
            "solutions": {
                // best batch
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)],
                        "Order 2": [amount(1_000), amount(1_100)]
                    },
                    "score": score(200),
                },
                // compatible batch
                "Solution 2": {
                    "solver": "Solver 1", // same solver
                    "trades": {
                        "Order 3": [amount(1_000), amount(1_100)],
                    },
                    "score": score(100),
                }
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2"],
            "expected_winners": ["Solution 1", "Solution 2"],
            "expected_reference_scores": {
                "Solver 1": "0",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    // Incompatible bid does not win but increases the reference score of the winner
    fn incompatible_bids() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["sell", "Token C", amount(1_000), "Token D", amount(1_000)],
                }
            },
            "solutions": {
                // best batch
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)],
                        "Order 2": [amount(1_000), amount(1_100)]
                    },
                    "score": score(200),
                },
                // compatible batch
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)],
                    },
                    "score": score(100),
                }
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2"],
            "expected_winners": ["Solution 1"],
            "expected_reference_scores": {
                "Solver 1": "100",
                "Solver 2": "200",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    // Unfair batch is filtered
    fn fairness_filtering() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
                ["Token C", address(2)],
                ["Token D", address(3)]
            ],
            "auction": {
                "orders": {
                    "Order 1": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["sell", "Token C", amount(1_000), "Token D", amount(1_000)],
                }
            },
            "solutions": {
                // unfair batch
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)],
                        "Order 2": [amount(1_000), amount(1_100)]
                    },
                    "score": score(200),
                },
                // filtering batch
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_150)],
                    },
                    "score": score(150),
                }
            },
            "expected_fair_solutions": ["Solution 2"],
            "expected_winners": ["Solution 2"],
            "expected_reference_scores": {
                "Solver 2": "0",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    // Multiple trades on the same (directed) token pair are aggregated for
    // filtering
    fn aggregation_on_token_pair() {
        let case = json!({
            "tokens": [
                ["Token A", address(0)],
                ["Token B", address(1)],
            ],
            "auction": {
                "orders": {
                    "Order 1": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                }
            },
            "solutions": {
                // batch with aggregation
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)],
                        "Order 2": [amount(1_000), amount(1_100)]
                    },
                    "score": score(200),
                },
                // incompatible batch
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_150)],
                    },
                    "score": score(150),
                }
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2"],
            "expected_winners": ["Solution 1"],
            "expected_reference_scores": {
                "Solver 1": "150",
                "Solver 2": "200",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    // Reference winners can generate more surplus than winners
    fn reference_better_than_winners() {
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
                    "Order 1": ["sell", "Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["sell", "Token C", amount(1_000), "Token D", amount(1_000)],
                    "Order 3": ["sell", "Token E", amount(1_000), "Token F", amount(1_000)],
                }
            },
            "solutions": {
                // best batch
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_100)],
                        "Order 2": [amount(1_000), amount(1_100)],
                        "Order 3": [amount(1_000), amount(1_100)]
                    },
                    "score": score(300),
                },
                // incompatible batch 1
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 1": [amount(1_000), amount(1_140)],
                        "Order 2": [amount(1_000), amount(1_140)],
                    },
                    "score": score(280),
                },
                // incompatible batch 2
                "Solution 3": {
                    "solver": "Solver 3",
                    "trades": {
                        "Order 3": [amount(1_000), amount(1_100)],
                    },
                    "score": score(100),
                }
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2",  "Solution 3"],
            "expected_winners": ["Solution 1"],
            "expected_reference_scores": {
                "Solver 1": "380",
                "Solver 2": "300",
                "Solver 3": "300",
            },
        });
        TestCase::from_json(case).validate();
    }

    #[test]
    fn staging_mainnet_auction_12825008() {
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
                    "Order 1": ["sell", "Token A", "32375066190000000000000000", "Token B", "2161512119"],
                },
                "prices": {
                    "Token A": "32429355240",
                    "Token B": "480793239987749750742974464",
                }
            },
            "solutions": {
                // solution 1 (baseline, the winner)
                "Solution 1": {
                    "solver": "Solver 1",
                    "trades": {
                        "Order 1": ["32375066190000000000000000", "2206881314"],
                    },
                    "score": "21813202259686016",
                },
                // solution 2 (zeroex)
                "Solution 2": {
                    "solver": "Solver 2",
                    "trades": {
                        "Order 1": ["32375066190000000000000000", "2205267875"],
                    },
                    "score": "21037471695353421",
                },
            },
            "expected_fair_solutions": ["Solution 1", "Solution 2"],
            "expected_winners": ["Solution 1"],
            "expected_reference_scores": {
                "Solver 1": "21037471695353421",
                "Solver 2": "21813202259686016"
            },
        });
        TestCase::from_json(case).validate();
    }

    /* 
    #[test]
    fn staging_mainnet_auction_12825008_from_db() {
        let auction_id = 12825008;

        // Fetch auction data
        let auction = fetch_auction_orders_data(auction_id, auction_id);

        // Fetch solutions data
        let solutions = fetch_trade_data(auction_id, auction_id);

        let mut test_case = TestCase {
            tokens: vec![],
            auction,
            solutions,
            expected_fair_solutions: vec![],
            expected_winners: vec![],
            expected_reference_scores: HashMap::new(),
        };

        // Collect unique tokens from orders
        let mut unique_tokens = HashSet::new();
        for (_, order) in &test_case.auction.orders {
            // sell token
            unique_tokens.insert(order.1.clone());
            // buy token
            unique_tokens.insert(order.3.clone());
        }
        for token in unique_tokens {
            test_case
                .tokens
                .push((token.clone(), H160::from_str(&token).unwrap()));
        }

        eprintln!("{:#?}", test_case);

        test_case.validate();
    }
    */

    #[test]
    fn store_auction_results() {
        let auction_start = 12825008;
        let auction_end = 12825008;

        // fetch db data for all auctions
        let db_trade_data = fetch_trade_data(auction_start, auction_end);
        let db_auction_data = fetch_auction_orders_data(auction_start, auction_end);

        // auction_id, winner, same winner as db, reference_score, num_winners, error message
        let mut results: Vec<(i64, String, bool, eth::U256, usize, Option<String>)> = Vec::new();

        for auction_id in auction_start..=auction_end {
            let result = match (db_trade_data.get(&auction_id), db_auction_data.get(&auction_id)) {
                (Some(solutions), Some(auction)) => {
                    let mut test_case = TestCase {
                        tokens: vec![],
                        auction: auction.clone(),
                        solutions: solutions.clone(),
                        expected_fair_solutions: vec![],
                        expected_winners: vec![],
                        expected_reference_scores: HashMap::new(),
                    };

                    // Collect unique tokens from orders
                    let mut unique_tokens = HashSet::new();
                    for (_, order) in &test_case.auction.orders {
                        // sell token
                        unique_tokens.insert(order.1.clone());
                        // buy token
                        unique_tokens.insert(order.3.clone());
                    }
                    for token in unique_tokens {
                        test_case
                            .tokens
                            .push((token.clone(), H160::from_str(&token).unwrap()));
                    }

                    match test_case.calculate_results(auction_id) {
                        Ok(result) => result,
                        Err(e) => (auction_id, String::new(), false, eth::U256::zero(), 0, Some(e.to_string())),
                    }
                }
                _ => (auction_id, String::new(), false, eth::U256::zero(), 0, Some("Missing auction or trade data".to_string())),
            };
            results.push(result);
        }

        eprintln!("Results:");
        for (auction_id, winner, same_winner, reference_score, num_winners, error) in &results {
            if let Some(err) = error {
                eprintln!("Auction {}: Error - {}", auction_id, err);
            } else {
                eprintln!("Auction {}: Winner={}, Same as DB={}, Reference Score={}, Number of Winners={}", 
                    auction_id, winner, same_winner, reference_score, num_winners);
            }
        }
    }

    impl TestCase {
        pub fn calculate_results(&self, auction_id: i64) -> Result<(i64, String, bool, eth::U256, usize, Option<String>), String> {
            let arbitrator = create_test_arbitrator();

            // map (token id -> token address) for later reference during the test
            let token_map: HashMap<String, H160> = self.tokens.iter().cloned().collect();

            // map (order id -> order) for later reference during the test
            let order_map: HashMap<String, Order> = self
                .auction
                .orders
                .iter()
                .map(
                    |(
                        order_id,
                        TestOrder(side, sell_token, sell_token_amount, buy_token, buy_token_amount),
                    )| {
                        let order_uid = hash(order_id);
                        let sell_token = token_map.get(sell_token).unwrap();
                        let buy_token = token_map.get(buy_token).unwrap();
                        let order = create_order(
                            order_uid,
                            *sell_token,
                            *sell_token_amount,
                            *buy_token,
                            *buy_token_amount,
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
                        let token_address = TokenAddress(*token_map.get(token_id).unwrap());
                        let price = create_price(*price);
                        (token_address, price)
                    })
                    .collect()
            });

            let auction = create_auction(orders, prices);

            // map (solver id -> solver address) for later reference during the test
            let mut solver_map = HashMap::new();

            // map (solution id -> participant) for later reference during the test
            let mut solution_map = HashMap::new();
            for (solution_id, solution) in &self.solutions {
                let solver_address = H160::from_str(&solution.solver).unwrap();
                solver_map.insert(solution.solver.clone(), solver_address);

                let trades = solution
                    .trades
                    .iter()
                    .map(|(order_id, trade)| {
                        let order = order_map.get(order_id).unwrap();
                        let sell_token_amount = trade.0;
                        let buy_token_amount = trade.1;
                        let trade = create_trade(order, sell_token_amount, buy_token_amount);
                        (order.uid, trade)
                    })
                    .collect();

                let solution_uid = hash(solution_id);
                solution_map.insert(
                    solution_id,
                    create_solution(solution_uid, solver_address, solution.score, trades, None),
                );
            }

            // filter solutions
            let participants = solution_map.values().cloned().collect();
            let solutions = arbitrator.filter_unfair_solutions(participants, &auction);

            // select the winners
            let solutions = arbitrator.mark_winners(solutions);
            let winners = filter_winners(&solutions);

            // Get the winner from our calculation
            let calculated_winner = winners.first()
                .map(|w| hex::encode(w.driver().submission_address.0))
                .unwrap_or_default();

            // Get the winner from the database
            let db_winner = self.solutions.iter()
                .find(|(_, solution)| solution.is_winner)
                .map(|(_, solution)| solution.solver.clone())
                .unwrap_or_default();

            // compute reference score
            let reference_scores = arbitrator.compute_reference_scores(&solutions);
            let reference_score = reference_scores.values()
                .next()
                .map(|score| score.get().0)
                .unwrap_or_default();

            Ok((
                auction_id,
                calculated_winner.clone(),
                calculated_winner == db_winner,
                reference_score,
                winners.len(),
                None,
            ))
        }
    }

    fn fetch_trade_data(
        start_auction_id: i64,
        end_auction_id: i64,
    ) -> HashMap<i64, HashMap<String, TestSolution>> {
        use {
            dotenv::dotenv,
            sqlx::{PgPool, Row, types::BigDecimal},
            std::collections::HashMap,
        };

        // Load environment variables from .env file
        dotenv().ok();

        // Create a simple async block to run the database query
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Get database URL from environment
            let database_url =
                std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");

            // Create connection pool
            let pool = PgPool::connect(&database_url)
                .await
                .expect("Failed to create connection pool");

            let query = format!(
                "WITH trade_data AS (
                    SELECT 
                        ps.*,
                        pte.order_uid,
                        COALESCE(o.sell_token, pjo.sell_token) AS sell_token,
                        COALESCE(o.buy_token, pjo.buy_token) AS buy_token,
                        pte.executed_sell AS executed_sell_amount,
                        pte.executed_buy AS executed_buy_amount,
                        COALESCE(o.sell_amount, pjo.limit_sell) AS limit_sell_amount,
                        COALESCE(o.buy_amount, pjo.limit_buy) AS limit_buy_amount,
                        COALESCE(o.kind, pjo.side) AS kind
                    FROM proposed_solutions ps
                    LEFT JOIN proposed_trade_executions pte
                        ON ps.auction_id = pte.auction_id 
                        AND ps.uid = pte.solution_uid
                    LEFT JOIN orders o
                        ON pte.order_uid = o.uid
                    LEFT JOIN proposed_jit_orders pjo
                        ON ps.auction_id = pjo.auction_id
                        AND ps.uid = pjo.solution_uid
                        AND pte.order_uid = pjo.order_uid
                    WHERE ps.auction_id BETWEEN {start_auction_id} AND {end_auction_id}
                ),
                trade_data_with_prices AS (
                    SELECT 
                        td.*,
                        ap_sell.price AS sell_token_price,
                        ap_buy.price AS buy_token_price,
                        ps.solver,
                        ps.is_winner
                    FROM trade_data td
                    JOIN auction_prices ap_sell
                        ON td.auction_id = ap_sell.auction_id
                        AND td.sell_token = ap_sell.token
                    JOIN auction_prices ap_buy
                        ON td.auction_id = ap_buy.auction_id
                        AND td.buy_token = ap_buy.token
                    JOIN proposed_solutions ps
                        ON td.auction_id = ps.auction_id
                        AND td.uid = ps.uid
                )
                SELECT *
                FROM trade_data_with_prices;"
            );

            // Execute the provided query using sqlx
            match sqlx::query(&query).fetch_all(&pool).await {
                Ok(rows) => {
                    let mut auction_solutions: HashMap<i64, HashMap<String, TestSolution>> = HashMap::new();

                    for row in rows {
                        let auction_id = row.get::<i64, _>("auction_id");
                        let solver = hex::encode(row.get::<[u8; 20], _>("solver"));
                        let order_uid = hex::encode(row.get::<[u8; 56], _>("order_uid"));
                        let is_winner = row.get::<bool, _>("is_winner");

                        let mut trades = HashMap::new();
                        trades.insert(
                            order_uid,
                            TestTrade(
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("executed_sell_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("executed_buy_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                            ),
                        );

                        let score = eth::U256::from_str_radix(
                            &row.get::<BigDecimal, _>("score").to_string(),
                            10,
                        )
                        .unwrap();

                        let solution = TestSolution {
                            solver: solver.clone(),
                            trades,
                            score,
                            is_winner,
                        };

                        // Get or create the solutions map for this auction
                        let solutions = auction_solutions
                            .entry(auction_id)
                            .or_insert_with(HashMap::new);

                        // Insert or update the solution for this solver
                        solutions.insert(solver, solution);
                    }
                    auction_solutions
                }
                Err(e) => {
                    eprintln!("Query error: {}", e);
                    HashMap::new()
                }
            }
        })
    }

    fn fetch_auction_orders_data(start_auction_id: i64, end_auction_id: i64) -> HashMap<i64, TestAuction> {
        use {
            dotenv::dotenv,
            sqlx::{PgPool, Row, types::BigDecimal},
            std::collections::HashMap,
        };

        // Load environment variables from .env file
        dotenv().ok();

        // Create a simple async block to run the database query
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Get database URL from environment
            let database_url =
                std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");

            // Create connection pool
            let pool = PgPool::connect(&database_url)
                .await
                .expect("Failed to create connection pool");

            let query = format!(
                "WITH auction_orders AS (
                    SELECT 
                        ao.auction_id,
                        unnest(ao.order_uids) as order_uid
                    FROM auction_orders ao
                    WHERE ao.auction_id BETWEEN {start_auction_id} AND {end_auction_id}
                )
                SELECT 
                    ao.auction_id,
                    ao.order_uid,
                    o.kind::text as side,
                    o.sell_token,
                    o.buy_token,
                    o.sell_amount,
                    o.buy_amount,
                    ap_sell.price as sell_token_price,
                    ap_buy.price as buy_token_price
                FROM auction_orders ao
                JOIN orders o ON ao.order_uid = o.uid
                JOIN auction_prices ap_sell ON ao.auction_id = ap_sell.auction_id AND o.sell_token = ap_sell.token
                JOIN auction_prices ap_buy ON ao.auction_id = ap_buy.auction_id AND o.buy_token = ap_buy.token
                ORDER BY ao.auction_id, ao.order_uid;"
            );

            // Execute the provided query using sqlx
            match sqlx::query(&query).fetch_all(&pool).await {
                Ok(rows) => {
                    let mut auction_data: HashMap<i64, TestAuction> = HashMap::new();

                    for row in rows {
                        let auction_id = row.get::<i64, _>("auction_id");
                        let order_uid = hex::encode(row.get::<[u8; 56], _>("order_uid"));
                        let sell_token = hex::encode(row.get::<[u8; 20], _>("sell_token"));
                        let buy_token = hex::encode(row.get::<[u8; 20], _>("buy_token"));

                        // Get or create the TestAuction for this auction_id
                        let auction = auction_data
                            .entry(auction_id)
                            .or_insert_with(|| TestAuction {
                                orders: HashMap::new(),
                                prices: Some(HashMap::new()),
                            });

                        // Create order entry
                        auction.orders.insert(
                            order_uid.clone(),
                            TestOrder(
                                match row.get::<String, _>("side").to_lowercase().as_str() {
                                    "buy" => order::Side::Buy,
                                    "sell" => order::Side::Sell,
                                    _ => panic!(
                                        "Invalid side value: {}",
                                        row.get::<String, _>("side")
                                    ),
                                },
                                sell_token.clone(),
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("sell_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                                buy_token.clone(),
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("buy_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                            ),
                        );

                        // Add token prices if not already present
                        if let Some(prices) = &mut auction.prices {
                            if !prices.contains_key(&sell_token) {
                                prices.insert(
                                    sell_token,
                                    eth::U256::from_str_radix(
                                        &row.get::<BigDecimal, _>("sell_token_price").to_string(),
                                        10,
                                    )
                                    .unwrap(),
                                );
                            }
                            if !prices.contains_key(&buy_token) {
                                prices.insert(
                                    buy_token,
                                    eth::U256::from_str_radix(
                                        &row.get::<BigDecimal, _>("buy_token_price").to_string(),
                                        10,
                                    )
                                    .unwrap(),
                                );
                            }
                        }
                    }

                    auction_data
                }
                Err(e) => {
                    eprintln!("Query error: {}", e);
                    HashMap::new()
                }
            }
        })
    }

    #[serde_as]
    #[derive(Deserialize, Debug)]
    struct TestCase {
        pub tokens: Vec<(String, H160)>,
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

        pub fn validate(&self) {
            let arbitrator = create_test_arbitrator();

            // map (token id -> token address) for later reference during the test
            let token_map: HashMap<String, H160> = self.tokens.iter().cloned().collect();

            // map (order id -> order) for later reference during the test
            let order_map: HashMap<String, Order> = self
                .auction
                .orders
                .iter()
                .map(
                    |(
                        order_id,
                        TestOrder(side, sell_token, sell_token_amount, buy_token, buy_token_amount),
                    )| {
                        let order_uid = hash(order_id);
                        let sell_token = token_map.get(sell_token).unwrap();
                        let buy_token = token_map.get(buy_token).unwrap();
                        let order = create_order(
                            order_uid,
                            *sell_token,
                            *sell_token_amount,
                            *buy_token,
                            *buy_token_amount,
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
                        let token_address = TokenAddress(*token_map.get(token_id).unwrap());
                        let price = create_price(*price);
                        (token_address, price)
                    })
                    .collect()
            });

            let auction = create_auction(orders, prices);

            // map (solver id -> solver address) for later reference during the test
            let mut solver_map = HashMap::new();

            // map (solution id -> participant) for later reference during the test
            let mut solution_map = HashMap::new();
            for (solution_id, solution) in &self.solutions {
                let solver_address = H160::from_str(&solution.solver).unwrap();
                solver_map.insert(solution.solver.clone(), solver_address);

                let trades = solution
                    .trades
                    .iter()
                    .map(|(order_id, trade)| {
                        let order = order_map.get(order_id).unwrap();
                        let sell_token_amount = trade.0;
                        let buy_token_amount = trade.1;
                        let trade = create_trade(order, sell_token_amount, buy_token_amount);
                        (order.uid, trade)
                    })
                    .collect();

                let solution_uid = hash(solution_id);
                solution_map.insert(
                    solution_id,
                    create_solution(solution_uid, solver_address, solution.score, trades, None),
                );
            }

            // filter solutions
            let participants = solution_map.values().cloned().collect();
            let solutions = arbitrator.filter_unfair_solutions(participants, &auction);

            // select the winners
            let solutions = arbitrator.mark_winners(solutions);
            let winners = filter_winners(&solutions);

            // Check that winners match the database
            let _expected_winners: Vec<H160> = self
                .solutions
                .iter()
                .filter(|(_, solution)| solution.is_winner)
                .map(|(_, solution)| solution.solver.clone())
                .map(|solver| H160::from_str(&solver).unwrap())
                .collect();

            // compute reference score
            let _reference_scores = arbitrator.compute_reference_scores(&solutions);
            //eprintln!("Reference scores: {:#?}", reference_scores);
        }
    }

    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    struct TestAuction {
        pub orders: HashMap<String, TestOrder>,
        #[serde(default)]
        #[serde_as(as = "Option<HashMap<_, HexOrDecimalU256>>")]
        pub prices: Option<HashMap<String, eth::U256>>,
    }

    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    struct TestOrder(
        // side,
        #[serde(deserialize_with = "deserialize_side")] pub order::Side,
        // sell_token
        pub String,
        // sell_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
        // buy_token
        pub String,
        // buy_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
    );

    #[derive(Deserialize, Debug, Clone)]
    struct TestSolution {
        pub solver: String,
        pub trades: HashMap<String, TestTrade>,
        pub score: eth::U256,
        #[serde(default)]
        pub is_winner: bool,
    }

    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    struct TestTrade(
        // sell_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
        // buy_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
    );

    fn create_test_arbitrator() -> super::Config {
        super::Config {
            max_winners: 10,
            weth: H160::from_slice(&hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")).into(),
        }
    }

    fn address(id: u64) -> H160 {
        H160::from_low_u64_le(id)
    }

    fn create_order(
        uid: u64,
        sell_token: H160,
        sell_amount: eth::U256,
        buy_token: H160,
        buy_amount: eth::U256,
        side: order::Side,
    ) -> Order {
        Order {
            uid: create_order_uid(uid),
            sell: eth::Asset {
                amount: sell_amount.into(),
                token: sell_token.into(),
            },
            buy: eth::Asset {
                amount: buy_amount.into(),
                token: buy_token.into(),
            },
            protocol_fees: vec![],
            side,
            receiver: None,
            owner: Default::default(),
            partially_fillable: false,
            executed: eth::U256::zero().into(),
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
            let default_price = create_price(DEFAULT_TOKEN_PRICE.into());
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
            side: order::Side::Sell,
            sell: order.sell,
            buy: order.buy,
            executed_sell: executed_sell.into(),
            executed_buy: executed_buy.into(),
        }
    }

    fn create_solution(
        solution_id: u64,
        solver_address: H160,
        score: eth::U256,
        trades: Vec<(OrderUid, TradedOrder)>,
        prices: Option<HashMap<TokenAddress, Price>>,
    ) -> Participant<Unranked> {
        // The prices of the tokens do not affect the result but they keys must exist
        // for every token of every trade
        let prices = prices.unwrap_or({
            let mut res = HashMap::new();
            for (_, trade) in &trades {
                res.insert(trade.buy.token, create_price(eth::U256::one()));
                res.insert(trade.sell.token, create_price(eth::U256::one()));
            }
            res
        });

        let trade_order_map: HashMap<OrderUid, TradedOrder> = trades.into_iter().collect();
        let solver_address = eth::Address(solver_address);

        let solution = Solution::new(
            solution_id,
            solver_address,
            Score(eth::Ether(score)),
            trade_order_map,
            prices,
        );

        Participant::new(
            solution,
            Driver::mock(solver_address.to_string(), solver_address).into(),
        )
    }

    fn amount(value: u128) -> String {
        // adding decimal units to avoid the math rounding it down to 0
        to_e15(value).to_string()
    }

    fn to_e15(value: u128) -> u128 {
        value * 10u128.pow(15)
    }

    fn score(score: u128) -> String {
        // adding decimal units to avoid the math rounding it down to 0
        let score: u128 = to_e15(score);
        eth::U256::from(score)
            // Scores must be denominated in buy token price
            .checked_mul(DEFAULT_TOKEN_PRICE.into()).unwrap()
            // and expresed in wei
            .checked_div(10_u128.pow(18).into()).unwrap()
            .to_string()
    }

    fn filter_winners(solutions: &[Participant]) -> Vec<&Participant> {
        solutions.iter().filter(|s| s.is_winner()).collect()
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
            _ => Err(serde::de::Error::custom(format!("Invalid side: {}", s))),
        }
    }
}
