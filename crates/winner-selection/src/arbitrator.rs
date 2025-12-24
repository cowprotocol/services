//! Winner selection arbitrator.
//!
//! Implements the auction winner selection algorithm that picks the set of
//! solutions which maximize surplus while enforcing uniform directional
//! clearing prices.

use {
    crate::{
        auction::AuctionContext,
        primitives::{DirectedTokenPair, Price, Score, Side, TokenAmount, WrappedNativeToken},
        solution::{Order, Solution},
    },
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    itertools::{Either, Itertools},
    num::Saturating,
    std::collections::{HashMap, HashSet},
};

/// Auction arbitrator responsible for selecting winning solutions.
pub struct Arbitrator {
    /// Maximum number of winning solutions to select.
    pub max_winners: usize,
    /// Wrapped native token address (WETH on mainnet, WXDAI on Gnosis).
    pub weth: WrappedNativeToken,
}

impl Arbitrator {
    /// Runs the auction mechanism on solutions.
    ///
    /// Takes solutions and auction context, returns a ranking with winners.
    pub fn arbitrate<FeePolicy>(
        &self,
        mut solutions: Vec<Solution>,
        context: &AuctionContext<FeePolicy>,
    ) -> Ranking
    where
        FeePolicy: AsRef<[u8]>, // Placeholder - will be refined when integrating
    {
        // Compute scores and filter out invalid solutions
        let scores_by_solution = self.compute_scores(&mut solutions, context);

        // Compute total scores for sorting
        let total_scores: HashMap<u64, Score> = scores_by_solution
            .iter()
            .map(|(id, scores_by_pair)| {
                let total = scores_by_pair
                    .values()
                    .copied()
                    .reduce(Score::saturating_add)
                    .unwrap_or_default();
                (*id, total)
            })
            .collect();

        // Sort by score descending
        solutions.sort_by_key(|solution| {
            std::cmp::Reverse(total_scores.get(&solution.id).copied().unwrap_or_default())
        });

        let baseline_scores = compute_baseline_scores(&scores_by_solution);

        // Partition into fair and unfair solutions
        let (fair, unfair): (Vec<_>, Vec<_>) = solutions.into_iter().partition_map(|solution| {
            let aggregated_scores = match scores_by_solution.get(&solution.id) {
                Some(scores) => scores,
                None => return Either::Right(solution), // Filtered out
            };

            // Only filter out unfair solutions with more than one token pair
            if aggregated_scores.len() == 1
                || aggregated_scores.iter().all(|(pair, score)| {
                    baseline_scores
                        .get(pair)
                        .is_none_or(|baseline| score >= baseline)
                })
            {
                Either::Left(solution)
            } else {
                Either::Right(solution)
            }
        });

        // Pick winners from fair solutions
        let winner_indices = self.pick_winners(fair.iter());
        let ranked: Vec<_> = fair
            .into_iter()
            .enumerate()
            .map(|(index, solution)| RankedSolution {
                solution,
                is_winner: winner_indices.contains(&index),
            })
            .collect();

        Ranking {
            filtered_out: unfair,
            ranked,
        }
    }

    /// Compute scores for all solutions.
    fn compute_scores<FeePolicy>(
        &self,
        solutions: &mut [Solution],
        context: &AuctionContext<FeePolicy>,
    ) -> HashMap<u64, HashMap<DirectedTokenPair, Score>> {
        let mut scores = HashMap::new();

        for solution in solutions.iter() {
            let mut solution_scores = HashMap::new();

            for order in &solution.orders {
                // Only score orders that contribute to the solution's score
                if !context.contributes_to_score(&order.uid) {
                    continue;
                }

                // Compute score for this order
                match self.compute_order_score(order, solution, context) {
                    Ok(score) => {
                        let pair = DirectedTokenPair {
                            sell: order.sell_token.as_erc20(self.weth),
                            buy: order.buy_token.as_erc20(self.weth),
                        };
                        solution_scores
                            .entry(pair)
                            .or_insert(Score::default())
                            .saturating_add_assign(score);
                    }
                    Err(err) => {
                        tracing::warn!(
                            ?order.uid,
                            ?err,
                            "failed to compute score for order, skipping solution"
                        );
                        // Return empty scores for this solution (will be filtered out)
                        scores.insert(solution.id, HashMap::new());
                        continue;
                    }
                }
            }

            scores.insert(solution.id, solution_scores);
        }

        scores
    }

    /// Compute score for a single order.
    ///
    /// Score = surplus + protocol_fees (converted to native token).
    fn compute_order_score<FeePolicy>(
        &self,
        order: &Order,
        solution: &Solution,
        context: &AuctionContext<FeePolicy>,
    ) -> Result<Score> {
        // Get native prices for the tokens
        let _native_price_sell = context
            .native_prices
            .get(&order.sell_token)
            .context("missing native price for sell token")?;
        let native_price_buy = context
            .native_prices
            .get(&order.buy_token)
            .context("missing native price for buy token")?;

        // Get uniform clearing prices (native prices of the tokens)
        let uniform_sell_price = solution
            .prices
            .get(&order.sell_token)
            .context("missing uniform clearing price for sell token")?;
        let uniform_buy_price = solution
            .prices
            .get(&order.buy_token)
            .context("missing uniform clearing price for buy token")?;

        // Calculate surplus over limit price
        let surplus = self.calculate_surplus(order, uniform_sell_price, uniform_buy_price)?;

        // TODO: Calculate protocol fees from fee policies
        // For now, just return the surplus
        // let fees = self.calculate_protocol_fees(order, context)?;

        // Convert surplus to native token based on order side
        let score_eth = match order.side {
            Side::Sell => {
                // Surplus for sell orders is in buy token, convert directly to ETH
                native_price_buy.in_eth(TokenAmount(surplus))
            }
            Side::Buy => {
                // Surplus for buy orders is in sell token, first convert to buy token amount
                // then to ETH using: buy_amount = surplus * buy_limit / sell_limit
                let surplus_in_buy_tokens =
                    surplus.saturating_mul(order.buy_amount.0) / order.sell_amount.0;
                native_price_buy.in_eth(TokenAmount(surplus_in_buy_tokens))
            }
        };

        Score::new(score_eth).context("zero score")
    }

    /// Calculate user surplus over limit price.
    ///
    /// Returns surplus in the "surplus token" (buy token for sell orders, sell
    /// token for buy orders).
    fn calculate_surplus(
        &self,
        order: &Order,
        uniform_sell_price: &Price,
        uniform_buy_price: &Price,
    ) -> Result<U256> {
        match order.side {
            Side::Sell => {
                // For sell orders, surplus = bought - limit_buy
                // bought = executed_sell * uniform_sell / uniform_buy
                let bought = order.executed_sell.0.saturating_mul(uniform_sell_price.0.0)
                    / uniform_buy_price.0.0;

                // Scale limit buy for partially fillable orders
                let limit_buy = order
                    .executed_sell
                    .0
                    .saturating_mul(order.buy_amount.0)
                    .saturating_add(order.sell_amount.0 - U256::from(1u64)) // Ceiling division
                    / order.sell_amount.0;

                bought
                    .checked_sub(limit_buy)
                    .context("negative surplus (unfair trade)")
            }
            Side::Buy => {
                // For buy orders, surplus = limit_sell - sold
                // sold = executed_buy * uniform_buy / uniform_sell
                let sold = order.executed_buy.0.saturating_mul(uniform_buy_price.0.0)
                    / uniform_sell_price.0.0;

                // Scale limit sell for partially fillable orders
                let limit_sell =
                    order.executed_buy.0.saturating_mul(order.sell_amount.0) / order.buy_amount.0;

                limit_sell
                    .checked_sub(sold)
                    .context("negative surplus (unfair trade)")
            }
        }
    }

    /// Pick winners based on directional token pairs.
    fn pick_winners<'a>(&self, solutions: impl Iterator<Item = &'a Solution>) -> HashSet<usize> {
        let mut already_swapped_token_pairs = HashSet::new();
        let mut winners = HashSet::default();

        for (index, solution) in solutions.enumerate() {
            if winners.len() >= self.max_winners {
                return winners;
            }

            let swapped_token_pairs: HashSet<DirectedTokenPair> = solution
                .orders
                .iter()
                .map(|order| DirectedTokenPair {
                    sell: order.sell_token.as_erc20(self.weth),
                    buy: order.buy_token.as_erc20(self.weth),
                })
                .collect();

            if swapped_token_pairs.is_disjoint(&already_swapped_token_pairs) {
                winners.insert(index);
                already_swapped_token_pairs.extend(swapped_token_pairs);
            }
        }

        winners
    }

    /// Compute reference scores for winning solvers.
    pub fn compute_reference_scores(&self, ranking: &Ranking) -> HashMap<Address, Score> {
        let mut reference_scores = HashMap::default();

        for ranked_solution in &ranking.ranked {
            let solver = ranked_solution.solution.solver;

            if reference_scores.len() >= self.max_winners {
                return reference_scores;
            }
            if reference_scores.contains_key(&solver) {
                continue;
            }
            if !ranked_solution.is_winner {
                continue;
            }

            // Compute score without this solver
            let solutions_without_solver = ranking
                .ranked
                .iter()
                .filter(|s| s.solution.solver != solver)
                .map(|s| &s.solution);

            let winner_indices = self.pick_winners(solutions_without_solver.clone());

            let score = solutions_without_solver
                .enumerate()
                .filter(|(index, _)| winner_indices.contains(index))
                .map(|(_, _solution)| Score::default()) // TODO: Get actual scores
                .reduce(Score::saturating_add)
                .unwrap_or_default();

            reference_scores.insert(solver, score);
        }

        reference_scores
    }
}

/// Compute baseline scores (best single-pair solutions).
fn compute_baseline_scores(
    scores_by_solution: &HashMap<u64, HashMap<DirectedTokenPair, Score>>,
) -> HashMap<DirectedTokenPair, Score> {
    let mut baseline_scores = HashMap::default();

    for scores in scores_by_solution.values() {
        let Ok((token_pair, score)) = scores.iter().exactly_one() else {
            continue;
        };

        let current_best = baseline_scores.entry(token_pair.clone()).or_default();
        if score > current_best {
            *current_best = *score;
        }
    }

    baseline_scores
}

/// A solution with its ranking status.
#[derive(Debug, Clone)]
pub struct RankedSolution {
    pub solution: Solution,
    pub is_winner: bool,
}

/// Final ranking of all solutions.
#[derive(Debug)]
pub struct Ranking {
    /// Solutions that were filtered out as unfair.
    pub filtered_out: Vec<Solution>,
    /// Solutions that passed fairness checks, ordered by score.
    pub ranked: Vec<RankedSolution>,
}

impl Ranking {
    /// All winning solutions.
    pub fn winners(&self) -> impl Iterator<Item = &Solution> {
        self.ranked
            .iter()
            .filter(|r| r.is_winner)
            .map(|r| &r.solution)
    }

    /// All non-winning solutions that weren't filtered out.
    pub fn non_winners(&self) -> impl Iterator<Item = &Solution> {
        self.ranked
            .iter()
            .filter(|r| !r.is_winner)
            .map(|r| &r.solution)
    }
}
