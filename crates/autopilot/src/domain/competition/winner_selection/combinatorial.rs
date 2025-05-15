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
        std::collections::HashMap,
    };

    const DEFAULT_TOKEN_PRICE: u128 = 1_000;

    #[test]
    // Only one bid submitted results in one winner with reward equal to score
    fn single_bid() {
        let arbitrator = create_test_arbitrator();

        let token_a = create_address(0);
        let token_b = create_address(1);
        let token_c = create_address(2);
        let token_d = create_address(3);

        // auction
        let order_1 = create_order(1, token_a, amount(1_000), token_b, amount(1_000));
        let order_2 = create_order(2, token_c, amount(1_000), token_d, amount(1_000));
        let auction = create_auction(vec![order_1.clone(), order_2.clone()], None);

        // solution 1
        let solver = create_address(10);
        let trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_100)),
            (&order_2, amount(1_000), amount(1_100)),
        ]);
        let score = score_to_units(amount(200), None);
        let solution = create_solution(0, solver, score, trades, None);

        // filter solutions
        let participants = vec![solution];
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 1);

        // select the winners
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 1);

        // compute reference score
        // it must be 0 because it's the only solver that participated
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        assert_eq!(reference_scores.len(), 1);
        let solver_score = reference_scores.get(&eth::Address(solver)).unwrap();
        assert_eq!(solver_score.0, eth::Ether(0.into()));
    }

    #[test]
    // Two compatible batches are both selected as winners
    fn compatible_bids() {
        let arbitrator = create_test_arbitrator();

        let token_a = create_address(0);
        let token_b = create_address(1);
        let token_c = create_address(2);
        let token_d = create_address(3);

        // auction
        let order_1 = create_order(1, token_a, amount(1_000), token_b, amount(1_000));
        let order_2 = create_order(2, token_c, amount(1_000), token_d, amount(1_000));
        let order_3 = create_order(3, token_a, amount(1_000), token_c, amount(1_000));
        let auction = create_auction(
            vec![order_1.clone(), order_2.clone(), order_3.clone()],
            None,
        );

        // solution 1
        let solver_1 = create_address(10);
        let solver_1_trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_100)),
            (&order_2, amount(1_000), amount(1_100)),
        ]);
        let solver_1_score = score_to_units(amount(200), None);
        let solver_1_solution = create_solution(0, solver_1, solver_1_score, solver_1_trades, None);

        // solution 2
        let solver_2 = create_address(11);
        let solver_2_trades = create_trades(vec![(&order_3, amount(1_000), amount(1_100))]);
        let solver_2_score = score_to_units(amount(100), None);
        let solver_2_solution = create_solution(1, solver_2, solver_2_score, solver_2_trades, None);

        // filter solutions
        let participants = vec![solver_1_solution, solver_2_solution];
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 2);

        // select the winners
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 2);

        // compute reference scores
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        assert_eq!(reference_scores.len(), 2);

        // check that the reference scores are the expected
        let solver_1_reference_score = reference_scores.get(&eth::Address(solver_1)).unwrap();
        assert_eq!(
            solver_1_reference_score.0,
            eth::Ether(score_to_units(amount(100), None))
        );
        let solver_2_reference_score = reference_scores.get(&eth::Address(solver_2)).unwrap();
        assert_eq!(
            solver_2_reference_score.0,
            eth::Ether(score_to_units(amount(200), None))
        );
    }

    #[test]
    // Multiple compatible bids by a single solver are aggregated in rewards
    fn multiple_solution_for_solver() {
        let arbitrator = create_test_arbitrator();

        let token_a = create_address(0);
        let token_b = create_address(1);
        let token_c = create_address(2);
        let token_d = create_address(3);

        // auction
        let order_1 = create_order(1, token_a, amount(1_000), token_b, amount(1_000));
        let order_2 = create_order(2, token_c, amount(1_000), token_d, amount(1_000));
        let order_3 = create_order(3, token_a, amount(1_000), token_d, amount(1_000));
        let auction = create_auction(
            vec![order_1.clone(), order_2.clone(), order_3.clone()],
            None,
        );

        // we are using the same solver for both solutions
        let solver = create_address(10);

        // solution 1
        let solver_1_trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_100)),
            (&order_2, amount(1_000), amount(1_100)),
        ]);
        let solver_1_score = score_to_units(amount(200), None);
        let solver_1_solution = create_solution(0, solver, solver_1_score, solver_1_trades, None);

        // solution 2
        let solver_2_trades = create_trades(vec![(&order_3, amount(1_000), amount(1_100))]);
        let solver_2_score = score_to_units(amount(100), None);
        let solver_2_solution = create_solution(1, solver, solver_2_score, solver_2_trades, None);

        // filter solutions
        let participants = vec![solver_1_solution, solver_2_solution];
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 2);

        // select the winners
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 2);

        // compute reference score
        // it must be 0 because it's the only solver that participated
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        assert_eq!(reference_scores.len(), 1);
        let solver_score = reference_scores.get(&eth::Address(solver)).unwrap();
        assert_eq!(solver_score.0, eth::Ether(0.into()));
    }

    #[test]
    // Incompatible bid does not win but reduces reward
    fn incompatible_bids() {
        let arbitrator = create_test_arbitrator();

        let token_a = create_address(0);
        let token_b = create_address(1);
        let token_c = create_address(2);
        let token_d = create_address(3);

        // auction
        let order_1 = create_order(1, token_a, amount(1_000), token_b, amount(1_000));
        let order_2 = create_order(2, token_c, amount(1_000), token_d, amount(1_000));
        let auction = create_auction(vec![order_1.clone(), order_2.clone()], None);

        // solution 1, best batch
        let solver_1 = create_address(10);
        let solver_1_trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_100)),
            (&order_2, amount(1_000), amount(1_100)),
        ]);
        let solver_1_score = score_to_units(amount(200), None);
        let solver_1_solution = create_solution(0, solver_1, solver_1_score, solver_1_trades, None);

        // solution 2, incompatible batch
        let solver_2 = create_address(11);
        let solver_2_trades = create_trades(vec![(&order_1, amount(1_000), amount(1_100))]);
        let solver_2_score = score_to_units(amount(100), None);
        let solver_2_solution = create_solution(1, solver_2, solver_2_score, solver_2_trades, None);

        // filter solutions
        let participants = vec![solver_1_solution, solver_2_solution];
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 2);
        assert_eq!(solutions[0].driver().submission_address.0, solver_1);

        // select the winners
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].driver().submission_address.0, solver_1);

        // compute reference scores
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        let solver_1_reference_score = reference_scores.get(&eth::Address(solver_1)).unwrap();
        assert_eq!(
            solver_1_reference_score.0,
            eth::Ether(score_to_units(amount(100), None))
        );
        let solver_2_reference_score = reference_scores.get(&eth::Address(solver_2)).unwrap();
        assert_eq!(
            solver_2_reference_score.0,
            eth::Ether(score_to_units(amount(200), None))
        );
    }

    #[test]
    // Unfair batch is filtered
    fn fairness_filtering() {
        let arbitrator = create_test_arbitrator();

        let token_a = create_address(0);
        let token_b = create_address(1);
        let token_c = create_address(2);
        let token_d = create_address(3);

        // auction
        let order_1 = create_order(1, token_a, amount(1_000), token_b, amount(1_000));
        let order_2 = create_order(2, token_c, amount(1_000), token_d, amount(1_000));
        let auction = create_auction(vec![order_1.clone(), order_2.clone()], None);

        // solution 1, unfair batch
        let solver_1 = create_address(10);
        let solver_1_trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_100)),
            (&order_2, amount(1_000), amount(1_100)),
        ]);
        let solver_1_score = score_to_units(amount(200), None);
        let solver_1_solution = create_solution(0, solver_1, solver_1_score, solver_1_trades, None);

        // solution 2, filtering batch
        let solver_2 = create_address(11);
        let solver_2_trades = create_trades(vec![(&order_1, amount(1_000), amount(1_150))]);
        let solver_2_score = score_to_units(amount(150), None);
        let solver_2_solution = create_solution(1, solver_2, solver_2_score, solver_2_trades, None);

        // filter solutions (the unfair solver was filtered out)
        let participants = vec![solver_1_solution, solver_2_solution];
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].driver().submission_address.0, solver_2);

        // select the winners
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].driver().submission_address.0, solver_2);

        // compute reference scores
        // it must be 0 because the unfair solver got filtered out
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        assert_eq!(reference_scores.len(), 1);
        let solver_score = reference_scores.get(&eth::Address(solver_2)).unwrap();
        assert_eq!(solver_score.0, eth::Ether(0.into()));
    }

    #[test]
    // Multiple trades on the same (directed) token pair are aggregated for
    // filtering
    fn aggregation_on_token_pair() {
        let arbitrator = create_test_arbitrator();

        let token_a = create_address(0);
        let token_b = create_address(1);

        // auction
        let order_1 = create_order(1, token_a, amount(1_000), token_b, amount(1_000));
        let order_2 = create_order(2, token_a, amount(1_000), token_b, amount(1_000));
        let auction = create_auction(vec![order_1.clone(), order_2.clone()], None);

        // solution 1, batch with aggregation
        let solver_1 = create_address(10);
        let solver_1_trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_100)),
            (&order_2, amount(1_000), amount(1_100)),
        ]);
        let solver_1_score = score_to_units(amount(200), None);
        let solver_1_solution = create_solution(0, solver_1, solver_1_score, solver_1_trades, None);

        // solution 2, incompatible batch
        let solver_2 = create_address(11);
        let solver_2_trades = create_trades(vec![(&order_1, amount(1_000), amount(1_150))]);
        let solver_2_score = score_to_units(amount(150), None);
        let solver_2_solution = create_solution(1, solver_2, solver_2_score, solver_2_trades, None);

        // filter solutions
        let participants = vec![solver_1_solution, solver_2_solution];
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 2);

        // select the winners
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].driver().submission_address.0, solver_1);

        // compute reference scores
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        let solver_1_reference_score = reference_scores.get(&eth::Address(solver_1)).unwrap();
        assert_eq!(
            solver_1_reference_score.0,
            eth::Ether(score_to_units(amount(150), None))
        );
        let solver_2_reference_score = reference_scores.get(&eth::Address(solver_2)).unwrap();
        assert_eq!(
            solver_2_reference_score.0,
            eth::Ether(score_to_units(amount(200), None))
        );
    }

    #[test]
    // Reference winners can generate more surplus than winners
    fn reference_better_than_winners() {
        let arbitrator = create_test_arbitrator();

        let token_a = create_address(0);
        let token_b = create_address(1);
        let token_c = create_address(2);
        let token_d = create_address(3);
        let token_e = create_address(4);
        let token_f = create_address(5);

        // auction
        let order_1 = create_order(1, token_a, amount(1_000), token_b, amount(1_000));
        let order_2 = create_order(2, token_c, amount(1_000), token_d, amount(1_000));
        let order_3 = create_order(3, token_e, amount(1_000), token_f, amount(1_000));
        let auction = create_auction(
            vec![order_1.clone(), order_2.clone(), order_3.clone()],
            None,
        );

        // solution 1, best batch
        let solver_1 = create_address(10);
        let solver_1_trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_100)),
            (&order_2, amount(1_000), amount(1_100)),
            (&order_3, amount(1_000), amount(1_100)),
        ]);
        let solver_1_score = score_to_units(amount(300), None);
        let solver_1_solution = create_solution(0, solver_1, solver_1_score, solver_1_trades, None);

        // solution 2, incompatible batch 1
        let solver_2 = create_address(11);
        let solver_2_trades = create_trades(vec![
            (&order_1, amount(1_000), amount(1_140)),
            (&order_2, amount(1_000), amount(1_140)),
        ]);
        let solver_2_score = score_to_units(amount(280), None);
        let solver_2_solution = create_solution(1, solver_2, solver_2_score, solver_2_trades, None);

        // solution 3, incompatible batch 2
        let solver_3 = create_address(12);
        let solver_3_trades = create_trades(vec![(&order_3, amount(1_000), amount(1_100))]);
        let solver_3_score = score_to_units(amount(100), None);
        let solver_3_solution = create_solution(2, solver_3, solver_3_score, solver_3_trades, None);

        // filter solutions
        let participants = vec![solver_1_solution, solver_2_solution, solver_3_solution];
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 3);

        // select the winners
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].driver().submission_address.0, solver_1);

        // compute reference scores
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        let solver_1_reference_score = reference_scores.get(&eth::Address(solver_1)).unwrap();
        assert_eq!(
            solver_1_reference_score.0,
            score_to_units(amount(380), None).into()
        );
        let solver_2_reference_score = reference_scores.get(&eth::Address(solver_2)).unwrap();
        assert_eq!(
            solver_2_reference_score.0,
            score_to_units(amount(300), None).into()
        );
        let solver_3_reference_score = reference_scores.get(&eth::Address(solver_3)).unwrap();
        assert_eq!(
            solver_3_reference_score.0,
            score_to_units(amount(300), None).into()
        );
    }

    #[test]
    fn staging_mainnet_auction_12825008() {
        // https://solver-instances.s3.eu-central-1.amazonaws.com/staging/mainnet/autopilot/12825008.json
        // The example is an auction with one order and two competing bids for it, one
        // having a better score than the other

        let arbitrator = create_test_arbitrator();

        // corresponding to 0x67466be17df832165f8c80a5a120ccc652bd7e69
        let token_a = create_address(0);
        // corresponding to 0xdac17f958d2ee523a2206206994597c13d831ec7
        let token_b = create_address(1);

        // auction has only one order
        let order_1 = create_order(0, token_a, 32375066190000000000000000, token_b, 2161512119);
        let prices = create_prices(vec![
            (token_a, 32429355240),
            (token_b, 480793239987749750742974464),
        ]);
        let auction = create_auction(vec![order_1.clone()], Some(prices));

        // solution 1 (baseline, the winner)
        let solver_1 = create_address(10);
        let solver_1_trades =
            create_trades(vec![(&order_1, 32375066190000000000000000, 2206881314)]);
        let solver_1_prices = create_prices(vec![
            (token_a, 2206881314),
            (token_b, 32197996266469695053980358),
        ]);
        let solver_1_solution = create_solution(
            0,
            solver_1,
            21813202259686016u128.into(),
            solver_1_trades,
            Some(solver_1_prices),
        );

        // solution 2 (zeroex)
        let solver_2 = create_address(11);
        let solver_2_trades =
            create_trades(vec![(&order_1, 32375066190000000000000000, 2205267875)]);
        let solver_2_prices = create_prices(vec![
            (token_a, 2205267875),
            (token_b, 32174456479260706991472089),
        ]);
        let solver_2_solution = create_solution(
            1,
            solver_2,
            21037471695353421u128.into(),
            solver_2_trades,
            Some(solver_2_prices),
        );

        // run the combinatorial auction
        let participants = vec![solver_1_solution, solver_2_solution];

        // both solutions are fair
        let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
        assert_eq!(solutions.len(), 2);

        // check that solver_1 is the winner
        let solutions = arbitrator.mark_winners(solutions);
        let winners = filter_winners(&solutions);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].driver().submission_address.0, solver_1);

        // check that solver_1 reference score is the expected value
        let reference_scores = arbitrator.compute_reference_scores(&solutions);
        let winner_score = reference_scores.get(&eth::Address(solver_1)).unwrap();
        assert_eq!(winner_score.0, eth::Ether(21037471695353421u128.into()));
    }

    fn create_test_arbitrator() -> super::Config {
        super::Config {
            max_winners: 10,
            weth: H160::from_slice(&hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")).into(),
        }
    }

    fn create_address(id: u64) -> H160 {
        H160::from_low_u64_le(id)
    }

    fn create_order(
        uid: usize,
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

    fn create_prices(token_price_pairs: Vec<(H160, u128)>) -> HashMap<eth::TokenAddress, Price> {
        token_price_pairs
            .into_iter()
            .map(|(token, price)| (token.into(), create_price(price)))
            .collect()
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

    fn create_trades(input: Vec<(&Order, u128, u128)>) -> Vec<(OrderUid, TradedOrder)> {
        input
            .into_iter()
            .map(|(order, sell, buy)| (order.uid, create_trade(order, sell, buy)))
            .collect()
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

    fn amount(value: u128) -> u128 {
        // adding decimal units to avoid the math rounding it down to 0
        value * 10u128.pow(15)
    }

    fn score_to_units(score: u128, buy_token_price: Option<u128>) -> eth::U256 {
        let buy_token_price = buy_token_price.unwrap_or(DEFAULT_TOKEN_PRICE);
        eth::U256::from(score)
            // Scores must be denominated in buy token price
            .checked_mul(buy_token_price.into()).unwrap()
            // and expresed in wei
            .checked_div(10_u128.pow(18).into()).unwrap()
    }

    fn filter_winners(solutions: &[Participant]) -> Vec<&Participant> {
        solutions.iter().filter(|s| s.is_winner()).collect()
    }
}
