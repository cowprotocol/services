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
        Auction,
        auction::Prices,
        competition::{Participant, Score, Solution, Unranked},
        eth::{self, WrappedNativeToken},
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
    fn mark_winners(&self, mut solutions: Vec<Participant<Unranked>>) -> Vec<Participant> {
        solutions.sort_unstable_by_key(|participant| {
            std::cmp::Reverse(participant.solution().score().get().0)
        });

        // Winners are selected one by one, starting from the best solution,
        // until `max_winners` are selected. A solution can only
        // win if none of the (sell_token, buy_token) pairs of the executed
        // orders have been covered by any previously selected winning solution.
        // In other words this enforces a uniform directional clearing price.
        let mut already_swapped_tokens_pairs = HashSet::new();
        let mut winners = 0;
        solutions
            .into_iter()
            .map(|participant| {
                let swapped_token_pairs = participant
                    .solution()
                    .orders()
                    .iter()
                    .map(|(_, order)| {
                        (
                            order.sell.token.as_erc20(self.weth),
                            order.buy.token.as_erc20(self.weth),
                        )
                    })
                    .collect::<HashSet<_>>();

                let is_winner = swapped_token_pairs.is_disjoint(&already_swapped_tokens_pairs)
                    && winners < self.max_winners;

                already_swapped_tokens_pairs.extend(swapped_token_pairs);
                winners += usize::from(is_winner);

                participant.rank(is_winner)
            })
            .collect()
    }

    fn compute_reference_scores(&self, solutions: &[Participant]) -> HashMap<eth::Address, Score> {
        let mut reference_scores = HashMap::default();

        for solution in solutions {
            let driver = solution.driver().submission_address;
            if reference_scores.contains_key(&driver) {
                // we already computed the reference score
                continue;
            }

            let solutions_without_solver = solutions
                .iter()
                .filter(|s| s.driver().submission_address != driver)
                .cloned()
                .map(|solution| solution.unrank())
                .collect();
            let ranked = self.mark_winners(solutions_without_solver);
            let score = ranked
                .iter()
                .filter(|s| s.is_winner())
                .fold(U256::zero(), |acc, s| acc + s.solution().score().0);
            let score = Score::try_new(eth::Ether(score)).unwrap_or_default();
            reference_scores.insert(driver, score);
        }

        reference_scores
    }

    fn filter_solutions(
        &self,
        mut solutions: Vec<Participant<Unranked>>,
        auction: &Auction,
    ) -> Vec<Participant<Unranked>> {
        let baseline_scores = compute_baseline_solutions(&solutions, &auction.prices);
        solutions.retain(|s| {
            let aggregated_scores = aggregate_scores(s.solution(), &auction.prices);
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

/// Let's call a solution that only trades 1 directed token pair a baseline
/// solution. Returns the best baseline solution (highest score) for
/// each token pair if one exists.
fn compute_baseline_solutions(
    solutions: &[Participant<Unranked>],
    prices: &Prices,
) -> HashMap<DirectedTokenPair, Score> {
    let mut baseline_solutions = HashMap::default();
    for solution in solutions {
        let aggregate_scores = aggregate_scores(solution.solution(), prices);
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
fn aggregate_scores(
    solution: &Solution,
    native_prices: &Prices,
) -> HashMap<DirectedTokenPair, Score> {
    let mut scores = HashMap::default();
    for order in solution.orders().values() {
        let token_pair = DirectedTokenPair {
            sell: order.sell.token,
            buy: order.buy.token,
        };
        // TODO compute score
        let score = Default::default();
        *scores.entry(token_pair).or_default() += score;
    }
    scores
}
