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
    ethcontract::U256,
    std::collections::{HashMap, HashSet},
};

pub struct Config {
    pub max_winners: usize,
    pub weth: WrappedNativeToken,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct DirectedTokenPair {
    sell: eth::TokenAddress,
    buy: eth::TokenAddress,
}

impl Arbitrator for Config {
    fn mark_winners(&self, mut participants: Vec<Participant<Unranked>>) -> Vec<Participant> {
        participants.sort_unstable_by_key(|participant| {
            std::cmp::Reverse(participant.solution().score().get().0)
        });
        let winners = self.pick_winners(participants.iter().map(|p| p.solution()));
        let mut marked: Vec<_> = participants
            .into_iter()
            .enumerate()
            .map(|(index, participant)| participant.rank(winners.contains(&index)))
            .collect();
        marked.sort_by_key(|participant| std::cmp::Reverse(participant.is_winner()));
        marked
    }

    fn compute_reference_scores(
        &self,
        participants: &[Participant],
    ) -> HashMap<eth::Address, Score> {
        let mut reference_scores = HashMap::default();

        for participant in participants {
            let driver = participant.driver().submission_address;
            if reference_scores.len() >= self.max_winners {
                // all winners have been processed
                return reference_scores;
            }
            if reference_scores.contains_key(&driver) {
                // we already computed the reference score
                continue;
            }

            let solutions_without_solver = participants
                .iter()
                .filter(|p| p.driver().submission_address != driver)
                .map(|p| p.solution());

            let winners = self.pick_winners(solutions_without_solver.clone());

            let score = solutions_without_solver
                .enumerate()
                .filter(|(index, _)| winners.contains(index))
                .fold(U256::zero(), |acc, (_, s)| acc + s.score().0);
            let score = Score::try_new(eth::Ether(score)).unwrap_or_default();
            reference_scores.insert(driver, score);
        }

        reference_scores
    }

    fn filter_solutions(
        &self,
        mut solutions: Vec<Participant<Unranked>>,
        auction: &domain::Auction,
    ) -> Vec<Participant<Unranked>> {
        let auction = Auction::from(auction);
        let baseline_scores = compute_baseline_scores(&solutions, &auction);
        solutions.retain(|s| {
            let aggregated_scores = aggregate_scores(s.solution(), &auction);
            // only keep solutions where each order execution is at least as good as
            // the baseline solution
            aggregated_scores.iter().all(|(pair, score)| {
                baseline_scores
                    .get(pair)
                    .is_none_or(|baseline| score >= baseline)
            })
        });
        solutions
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
fn compute_baseline_scores(
    participants: &[Participant<Unranked>],
    auction: &Auction,
) -> HashMap<DirectedTokenPair, Score> {
    let mut baseline_solutions = HashMap::default();
    for participant in participants {
        let aggregate_scores = aggregate_scores(participant.solution(), auction);
        if aggregate_scores.len() != 1 {
            // base solutions must contain exactly 1 directed token pair
            continue;
        }
        let (token_pair, score) = aggregate_scores.into_iter().next().unwrap();
        let current_best_score = baseline_solutions.entry(token_pair).or_default();
        if score > *current_best_score {
            *current_best_score = score;
        }
    }
    baseline_solutions
}

/// Returns the total scores for each directed token pair of the solution.
/// E.g. if a solution contains 3 orders like:
///     sell A for B with a score of 10
///     sell A for B with a score of 5
///     sell B for C with a score of 5
/// it will return a map like:
///     (A, B) => 15
///     (B, C) => 5
fn aggregate_scores(solution: &Solution, auction: &Auction) -> HashMap<DirectedTokenPair, Score> {
    let mut scores = HashMap::default();
    for (uid, trade) in solution.orders() {
        if !auction.contributes_to_score(uid) {
            continue;
        }

        let trade = math::Trade {
            uid: *uid,
            sell: trade.sell,
            buy: trade.buy,
            side: trade.side,
            executed: match trade.side {
                order::Side::Buy => TargetAmount(trade.executed_buy.into()),
                order::Side::Sell => TargetAmount(trade.executed_sell.into()),
            },
            // TODO: double check that these prices make sense
            // do we need to always set `uniform` to executed when an order is not surplus
            // capturing?
            prices: transaction::Prices {
                uniform: ClearingPrices {
                    sell: solution
                        .prices
                        .get(&trade.sell.token)
                        .map(|p| p.get().0)
                        .unwrap_or_else(|| trade.sell.amount.0),
                    buy: solution
                        .prices
                        .get(&trade.buy.token)
                        .map(|p| p.get().0)
                        .unwrap_or_else(|| trade.buy.amount.0),
                },
                custom: ClearingPrices {
                    sell: trade.sell.amount.into(),
                    buy: trade.buy.amount.into(),
                },
            },
        };
        let score = trade
            .score(&auction.fee_policies, auction.native_prices)
            .unwrap();

        // clearing prices can be looked up in the solution.prices
        // custom prices are equal to the executed amounts
        // then build a trade from it and compute the score
        let token_pair = DirectedTokenPair {
            sell: trade.sell.token,
            buy: trade.buy.token,
        };

        *scores.entry(token_pair).or_default() += Score(score);
    }
    scores
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

// TODO
// * double check comb auction algo details:
//     * how to treat orders which should not get surplus
//     * is the logic compatible with a max_winners config option?
// * see if the score computation can be re-used more elegantly
//     * check if the current impl is even correct
// * update solver competition logic with new scores
// * store new data in the DB
//
// * split changes into multiple PRs
//     * make score logic public (also update the interfaces)
//     * introduce Arbitrate trait and move current logic
//     * implement comb auctions logic
//     * update solver competitions data and DB calls
