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
            // the baseline solution
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
                Auction,
                Order,
                OrderUid,
                auction::{
                    Price,
                    order::{self, AppDataHash},
                },
                competition::{
                    Participant,
                    Score,
                    Solution,
                    TradedOrder,
                    Unranked,
                    winner_selection::Arbitrator,
                },
                eth::{self, TokenAddress},
                fee::{self},
            },
            infra::Driver,
        },
        ethcontract::H160,
        hex_literal::hex,
        serde::Deserialize,
        serde_json::json,
        serde_with::{DisplayFromStr, serde_as},
        std::{
            collections::HashMap,
            hash::{DefaultHasher, Hash, Hasher},
        },
    };

    const DEFAULT_TOKEN_PRICE: u128 = 1_000;

    #[test]
    // Only one bid submitted results in one winner with reward equal to score
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
                    "Order 1": ["Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["Token C", amount(1_000), "Token D", amount(1_000)]
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
    // Only one bid submitted results in one winner with reward equal to score
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
                    "Order 1": ["Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["Token C", amount(1_000), "Token D", amount(1_000)],
                    "Order 3": ["Token A", amount(1_000), "Token C", amount(1_000)]
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
    // Only one bid submitted results in one winner with reward equal to score
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
                    "Order 1": ["Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["Token C", amount(1_000), "Token D", amount(1_000)],
                    "Order 3": ["Token A", amount(1_000), "Token D", amount(1_000)]
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
    // Incompatible bid does not win but reduces reward
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
                    "Order 1": ["Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["Token C", amount(1_000), "Token D", amount(1_000)],
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
                    "Order 1": ["Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["Token C", amount(1_000), "Token D", amount(1_000)],
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
                    "Order 1": ["Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["Token A", amount(1_000), "Token B", amount(1_000)],
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
                    "Order 1": ["Token A", amount(1_000), "Token B", amount(1_000)],
                    "Order 2": ["Token C", amount(1_000), "Token D", amount(1_000)],
                    "Order 3": ["Token E", amount(1_000), "Token F", amount(1_000)],
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
                    "Order 1": ["Token A", "32375066190000000000000000", "Token B", "2161512119"],
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
            },
        });
        TestCase::from_json(case).validate();
    }

    #[serde_as]
    #[derive(Deserialize, Debug)]
    pub struct TestCase {
        pub tokens: Vec<(String, H160)>,
        pub auction: TestAuction,
        pub solutions: HashMap<String, TestSolution>,
        pub expected_fair_solutions: Vec<String>,
        pub expected_winners: Vec<String>,
        // workaround needed as JSON have a limitation on number size, so we accept numbers as
        // String
        #[serde_as(as = "HashMap<_, DisplayFromStr>")]
        pub expected_reference_scores: HashMap<String, u128>,
    }

    impl TestCase {
        pub fn from_json(value: serde_json::Value) -> Self {
            serde_json::from_value(value).unwrap()
        }

        pub fn validate(&self) {
            let arbitrator = create_test_arbitrator();

            // map (token id -> token address) for later reference during the test
            let token_map: HashMap<String, H160> = self.tokens.clone().into_iter().collect();

            // map (order id -> order) for later reference during the test
            let order_map: HashMap<String, Order> = self
                .auction
                .orders
                .clone()
                .into_iter()
                .map(
                    |(
                        order_id,
                        TestOrder(sell_token, sell_token_amount, buy_token, buy_token_amount),
                    )| {
                        let order_uid = hash(&order_id);
                        let sell_token = token_map.get(&sell_token).unwrap();
                        let buy_token = token_map.get(&buy_token).unwrap();
                        let order = create_order(
                            order_uid,
                            *sell_token,
                            sell_token_amount,
                            *buy_token,
                            buy_token_amount,
                        );
                        (order_id, order)
                    },
                )
                .collect();

            let orders = order_map.values().cloned().collect();
            let prices = match &self.auction.prices {
                Some(prices) => {
                    let price_map = prices
                        .iter()
                        .map(|(token_id, price)| {
                            let token_address = TokenAddress(*token_map.get(token_id).unwrap());
                            let price = create_price(*price);
                            (token_address, price)
                        })
                        .collect();
                    Some(price_map)
                }
                None => None,
            };

            let auction = create_auction(orders, prices);

            // map (solver id -> solver address) for later reference during the test
            let mut solver_map = HashMap::new();

            // map (solution id -> participant) for later reference during the test
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
            assert_eq!(solutions.len(), self.expected_fair_solutions.len());
            for solution_id in self.expected_fair_solutions.clone() {
                let solution_uid = solution_map.get(&solution_id).unwrap().solution().id;
                assert!(solutions.iter().any(|s| s.solution().id == solution_uid));
            }

            // select the winners
            let solutions = arbitrator.mark_winners(solutions);
            let winners = filter_winners(&solutions);
            assert_eq!(winners.len(), self.expected_winners.len());
            for solution_id in self.expected_winners.clone() {
                let solution_uid = solution_map.get(&solution_id).unwrap().solution().id;
                assert!(winners.iter().any(|s| s.solution().id == solution_uid));
            }

            // compute reference score
            let reference_scores = arbitrator.compute_reference_scores(&solutions);
            for (solver_id, expected_score) in self.expected_reference_scores.clone() {
                let solver_address: eth::Address = (*solver_map.get(&solver_id).unwrap()).into();
                let score = reference_scores.get(&solver_address).unwrap();
                assert_eq!(score.0, eth::Ether(expected_score.into()))
            }
        }
    }

    #[serde_as]
    #[derive(Deserialize, Debug)]
    pub struct TestAuction {
        pub orders: HashMap<String, TestOrder>,
        #[serde(default)]
        // workaround needed as JSON have a limitation on number size, so we accept numbers as
        // String
        #[serde_as(as = "Option<HashMap<_, DisplayFromStr>>")]
        pub prices: Option<HashMap<String, u128>>,
    }

    // Here we also need the workaround to define the numbers as strings in JSON
    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    pub struct TestOrder(
        pub String,                                  // sell_token
        #[serde_as(as = "DisplayFromStr")] pub u128, // sell_amount
        pub String,                                  // buy_token
        #[serde_as(as = "DisplayFromStr")] pub u128, // buy_amount
    );

    #[derive(Deserialize, Debug)]
    pub struct TestSolution {
        pub solver: String,
        pub trades: HashMap<String, TestTrade>,
        pub score: eth::U256,
    }

    // Here we also need the workaround to define the numbers as strings in JSON
    #[serde_as]
    #[derive(Deserialize, Debug)]
    pub struct TestTrade(
        #[serde_as(as = "DisplayFromStr")] pub u128,
        #[serde_as(as = "DisplayFromStr")] pub u128,
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
        sell_amount: u128,
        buy_token: H160,
        buy_amount: u128,
    ) -> Order {
        let mock_protocol_fees = vec![fee::Policy::Surplus {
            factor: fee::FeeFactor::try_from(0.0).unwrap(),
            max_volume_factor: fee::FeeFactor::try_from(0.0).unwrap(),
        }];

        // build the UID of the order
        let mut encoded_uid = [0u8; 56];
        let uid_bytes = uid.to_le_bytes();
        encoded_uid[..uid_bytes.len()].copy_from_slice(&uid_bytes);
        let encoded_uid = OrderUid(encoded_uid);

        Order {
            uid: encoded_uid,
            sell: eth::Asset {
                amount: eth::U256::from(sell_amount).into(),
                token: sell_token.into(),
            },
            buy: eth::Asset {
                amount: eth::U256::from(buy_amount).into(),
                token: buy_token.into(),
            },
            protocol_fees: mock_protocol_fees,
            side: order::Side::Sell,
            receiver: Some(H160::zero().into()),
            owner: H160::zero().into(),
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

    fn create_price(value: u128) -> Price {
        Price::try_new(eth::Ether(eth::U256::from(value))).unwrap()
    }

    fn create_auction(
        orders: Vec<Order>,
        prices: Option<HashMap<eth::TokenAddress, Price>>,
    ) -> Auction {
        // Initialize the prices of the tokens if they are not specified
        let prices = prices.unwrap_or({
            let default_price = create_price(DEFAULT_TOKEN_PRICE);
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

    fn create_trade(order: &Order, executed_sell: u128, executed_buy: u128) -> TradedOrder {
        TradedOrder {
            side: order::Side::Sell,
            sell: order.sell,
            buy: order.buy,
            executed_sell: eth::U256::from(executed_sell).into(),
            executed_buy: eth::U256::from(executed_buy).into(),
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
                res.insert(trade.buy.token, create_price(1));
                res.insert(trade.sell.token, create_price(1));
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
        (value * 10u128.pow(15)).to_string()
    }

    fn score(score: u128) -> String {
        let score: u128 = amount(score)
            .parse()
            .expect("Failed to parse score as u128");
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

    fn hash(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}
