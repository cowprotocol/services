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
        solutions: Vec<Solution>,
        context: &AuctionContext<FeePolicy>,
    ) -> Ranking
    where
        FeePolicy: AsRef<[u8]>, // Placeholder - will be refined when integrating
    {
        let partitioned = self.partition_unfair_solutions(solutions, context);
        let filtered_out = partitioned.discarded;
        let ranked = self.mark_winners(partitioned.kept);

        Ranking {
            filtered_out,
            ranked,
        }
    }

    /// Removes unfair solutions from the set of all solutions.
    fn partition_unfair_solutions<FeePolicy>(
        &self,
        mut solutions: Vec<Solution>,
        context: &AuctionContext<FeePolicy>,
    ) -> PartitionedSolutions
    where
        FeePolicy: AsRef<[u8]>,
    {
        // Discard all solutions where we can't compute the aggregate scores
        // accurately because the fairness guarantees heavily rely on them.
        let scores_by_solution = self.compute_scores_by_solution(&mut solutions, context);

        // Sort by score descending
        solutions.sort_by_key(|solution| std::cmp::Reverse(solution.score.unwrap_or_default()));

        let baseline_scores = compute_baseline_scores(&scores_by_solution);

        // Partition into fair and unfair solutions
        let (kept, discarded): (Vec<_>, Vec<_>) = solutions.into_iter().partition_map(|solution| {
            let aggregated_scores = scores_by_solution
                .get(&solution.id)
                .expect("every remaining solution has an entry");

            // Only keep solutions where each order execution is at least as good as
            // the baseline solution.
            // We only filter out unfair solutions with more than one token pair,
            // to avoid reference scores set to 0.
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

        PartitionedSolutions { kept, discarded }
    }

    /// Picks winners and marks all solutions.
    fn mark_winners(&self, solutions: Vec<Solution>) -> Vec<RankedSolution> {
        let winner_indices = self.pick_winners(solutions.iter());

        solutions
            .into_iter()
            .enumerate()
            .map(|(index, solution)| RankedSolution {
                is_winner: winner_indices.contains(&index),
                solution,
            })
            .collect()
    }

    /// Computes the `DirectedTokenPair` scores for all solutions and discards
    /// solutions as invalid whenever that computation is not possible.
    /// Solutions get discarded because fairness guarantees heavily
    /// depend on these scores being accurate.
    fn compute_scores_by_solution<FeePolicy>(
        &self,
        solutions: &mut Vec<Solution>,
        context: &AuctionContext<FeePolicy>,
    ) -> HashMap<u64, HashMap<DirectedTokenPair, Score>>
    where
        FeePolicy: AsRef<[u8]>,
    {
        let mut scores = HashMap::default();

        solutions.retain_mut(
            |solution| match self.score_by_token_pair(solution, context) {
                Ok(score) => {
                    let total_score = score
                        .values()
                        .fold(Score::default(), |acc, s| acc.saturating_add(*s));
                    scores.insert(solution.id, score);
                    solution.score = Some(total_score);
                    true
                }
                Err(err) => {
                    tracing::warn!(
                        solution_id = solution.id,
                        ?err,
                        "discarding solution where scores could not be computed"
                    );
                    false
                }
            },
        );

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
    fn score_by_token_pair<FeePolicy>(
        &self,
        solution: &Solution,
        context: &AuctionContext<FeePolicy>,
    ) -> Result<HashMap<DirectedTokenPair, Score>>
    where
        FeePolicy: AsRef<[u8]>,
    {
        let mut scores: HashMap<DirectedTokenPair, Score> = HashMap::default();

        for order in &solution.orders {
            if !context.contributes_to_score(&order.uid) {
                continue;
            }

            let score = self.compute_order_score(order, solution, context)?;

            let token_pair = DirectedTokenPair {
                sell: order.sell_token.as_erc20(self.weth),
                buy: order.buy_token.as_erc20(self.weth),
            };

            scores
                .entry(token_pair)
                .or_default()
                .saturating_add_assign(score);
        }

        Ok(scores)
    }

    /// Score defined as (surplus + protocol fees) first converted to buy
    /// amounts and then converted to the native token.
    ///
    /// Follows CIP-38 as the base of the score computation.
    ///
    /// Denominated in NATIVE token.
    fn compute_order_score<FeePolicy>(
        &self,
        order: &Order,
        solution: &Solution,
        context: &AuctionContext<FeePolicy>,
    ) -> Result<Score> {
        let native_price_buy = context
            .native_prices
            .get(&order.buy_token)
            .context("missing native price for buy token")?;

        let uniform_sell_price = solution
            .prices
            .get(&order.sell_token)
            .context("missing uniform clearing price for sell token")?;
        let uniform_buy_price = solution
            .prices
            .get(&order.buy_token)
            .context("missing uniform clearing price for buy token")?;

        // Calculate surplus in surplus token (buy token for sell orders, sell token for
        // buy orders)
        #[allow(clippy::let_and_return)]
        let surplus_in_surplus_token = {
            let user_surplus =
                self.calculate_surplus(order, uniform_sell_price, uniform_buy_price)?;

            // TODO: Add protocol fees from fee policies
            // let fees: U256 = self.protocol_fees(order, context)?
            //     .into_iter()
            //     .try_fold(U256::ZERO, |acc, fee| {
            //         acc.checked_add(fee).context("overflow adding fees")
            //     })?;
            // user_surplus.checked_add(fees).context("overflow adding fees to surplus")?

            // For now, just use user surplus without fees
            user_surplus
        };

        let score_eth = match order.side {
            // `surplus` of sell orders is already in buy tokens so we simply convert it to ETH
            Side::Sell => native_price_buy.in_eth(TokenAmount(surplus_in_surplus_token)),
            Side::Buy => {
                // `surplus` of buy orders is in sell tokens. We start with following formula:
                // buy_amount / sell_amount == buy_price / sell_price
                //
                // since `surplus` of buy orders is in sell tokens we convert to buy amount via:
                // buy_amount == (buy_price / sell_price) * surplus
                //
                // to avoid loss of precision because we work with integers we first multiply
                // and then divide:
                // buy_amount = surplus * buy_price / sell_price
                use alloy::primitives::{U512, ruint::UintTryFrom};

                let surplus_in_buy_tokens = surplus_in_surplus_token
                    .widening_mul(order.buy_amount.0)
                    .checked_div(U512::from(order.sell_amount.0))
                    .context("division by zero converting surplus to buy tokens")?;
                let surplus_in_buy_tokens: U256 = U256::uint_try_from(surplus_in_buy_tokens)
                    .map_err(|_| anyhow::anyhow!("overflow converting surplus to buy tokens"))?;

                // Afterwards we convert the buy token surplus to the native token.
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

/// Result of partitioning solutions into fair and unfair.
struct PartitionedSolutions {
    /// Solutions that passed fairness checks.
    kept: Vec<Solution>,
    /// Solutions that were filtered out as unfair.
    discarded: Vec<Solution>,
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
