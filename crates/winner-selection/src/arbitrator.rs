//! Winner selection arbitrator.
//!
//! Implements the auction winner selection algorithm that picks the set of
//! solutions which maximize surplus while enforcing uniform directional
//! clearing prices. The algorithm is written once, generic over the chain's
//! type vocabulary ([`ChainTypes`]).

use {
    crate::{
        auction::AuctionContext,
        chain::{Amount, ChainTypes, SaturatingAdd, Zero},
        evm::Evm,
        primitives::{DirectedTokenPair, Side},
        solution::{Order, RankType, Solution, Unscored},
        state::{Ranked, RankedItem, Scored, ScoredItem, UnscoredItem},
    },
    anyhow::{Context, Result},
    itertools::{Either, Itertools},
    protocol_fees::{protocol_fees_by_policy, surplus_over_limit},
    std::{
        cmp::Reverse,
        collections::{HashMap, HashSet},
    },
    tracing::instrument,
};

/// Auction arbitrator responsible for selecting winning solutions.
pub struct Arbitrator<C: ChainTypes = Evm> {
    /// Maximum number of winning solutions to select.
    pub max_winners: usize,
    /// Wrapped native token (WETH on mainnet, WXDAI on Gnosis, wSOL on
    /// Solana).
    pub wrapped_native: C::TokenId,
}

impl<C: ChainTypes> Arbitrator<C> {
    /// Runs the auction mechanism on solutions.
    ///
    /// Takes solutions and auction context, returns a ranking with winners.
    #[instrument(skip_all)]
    pub fn arbitrate(
        &self,
        solutions: Vec<Solution<Unscored, C>>,
        context: &AuctionContext<C>,
    ) -> Ranking<C> {
        let partitioned = self.partition_unfair_solutions(solutions, context);
        let filtered_out = partitioned
            .discarded
            .into_iter()
            .map(|s| s.with_rank(RankType::FilteredOut))
            .collect();
        let mut ranked = self.mark_winners(partitioned.kept, context);

        ranked.sort_by_key(|solution| (Reverse(solution.is_winner()), Reverse(solution.score())));

        Ranking {
            filtered_out,
            ranked,
        }
    }

    /// Removes unfair solutions from the set of all solutions.
    #[instrument(skip_all)]
    fn partition_unfair_solutions(
        &self,
        solutions: Vec<Solution<Unscored, C>>,
        context: &AuctionContext<C>,
    ) -> PartitionedSolutions<C> {
        // Discard all solutions where we can't compute the aggregate scores
        // accurately because the fairness guarantees heavily rely on them.
        let (mut solutions, scores_by_solution) =
            self.compute_scores_by_solution(solutions, context);

        // only keep solutions that settle at least 1 order that the mechanism
        // "cares about"
        solutions.retain(|solution| {
            solution
                .orders()
                .iter()
                .any(|order| context.contributes_to_score(&order.uid))
        });

        // Sort by score descending
        solutions.sort_by_key(|solution| Reverse(solution.score()));

        let baseline_scores = compute_baseline_scores::<C>(&scores_by_solution);

        // Partition into fair and unfair solutions
        let (kept, discarded): (Vec<_>, Vec<_>) = solutions.into_iter().partition_map(|solution| {
            let aggregated_scores = scores_by_solution
                .get(&SolutionKey {
                    solver: solution.solver(),
                    solution_id: solution.id(),
                })
                .expect("every remaining solution has an entry");

            // only keep solutions where each order execution is at least as
            // good as the baseline solution.
            // we only filter out unfair solutions with more than one token
            // pair, to avoid reference scores set to 0.
            // see https://github.com/fhenneke/comb_auctions/issues/2
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

    /// Picks winners and sorts all solutions where winners come before
    /// non-winners and higher scores come before lower scores.
    fn mark_winners(
        &self,
        solutions: Vec<Solution<Scored<C::Amount>, C>>,
        context: &AuctionContext<C>,
    ) -> Vec<Solution<Ranked<C::Amount>, C>> {
        let winner_indices = self.pick_winners(solutions.iter(), context);

        solutions
            .into_iter()
            .enumerate()
            .map(|(index, solution)| {
                let rank_type = if winner_indices.contains(&index) {
                    RankType::Winner
                } else {
                    RankType::NonWinner
                };
                solution.with_rank(rank_type)
            })
            .collect()
    }

    /// Computes the `DirectionalScores` for all solutions and discards
    /// solutions as invalid whenever that computation is not possible.
    /// Solutions get discarded because fairness guarantees heavily
    /// depend on these scores being accurate.
    fn compute_scores_by_solution(
        &self,
        solutions: Vec<Solution<Unscored, C>>,
        context: &AuctionContext<C>,
    ) -> (ScoredSolutions<C>, ScoresBySolution<C>) {
        let mut scores_by_solution = HashMap::default();
        let mut scored_solutions = Vec::new();

        for solution in solutions {
            match score_by_token_pair(&solution, context) {
                Ok(score) => {
                    let total_score = sum_by_pair::<C>(&score);
                    scores_by_solution.insert(
                        SolutionKey {
                            solver: solution.solver(),
                            solution_id: solution.id(),
                        },
                        score,
                    );
                    scored_solutions.push(solution.with_score(total_score));
                }
                Err(err) => {
                    tracing::warn!(
                        solution_id = solution.id(),
                        ?err,
                        "discarding solution where scores could not be computed"
                    );
                }
            }
        }

        (scored_solutions, scores_by_solution)
    }

    /// Returns indices of winning solutions.
    /// Assumes that `solutions` is sorted by score descendingly.
    /// This logic was moved into a helper function to avoid a ton of `.clone()`
    /// operations in `compute_reference_scores()`.
    fn pick_winners<'a, T: 'a>(
        &self,
        solutions: impl Iterator<Item = &'a Solution<T, C>>,
        context: &AuctionContext<C>,
    ) -> HashSet<usize> {
        // Winners are selected one by one, starting from the best solution,
        // until `max_winners` are selected. A solution can only
        // win if none of the (sell_token, buy_token) pairs of the executed
        // orders have been covered by any previously selected winning solution.
        // In other words this enforces a uniform **directional** clearing
        // price.
        //
        // Only orders that contribute to the score are considered. JIT
        // liquidity orders (e.g. from non surplus-capturing owners) are the
        // solver's own liquidity and are equivalent to using an AMM via an
        // interaction, so they must not block other solutions on their token
        // pair. This mirrors the fairness filtering which ignores them too.
        let mut already_swapped_token_pairs = HashSet::new();
        let mut winners = HashSet::default();

        for (index, solution) in solutions.enumerate() {
            if winners.len() >= self.max_winners {
                return winners;
            }

            let swapped_token_pairs: HashSet<DirectedTokenPair<C>> = solution
                .orders()
                .iter()
                .filter(|order| context.contributes_to_score(&order.uid))
                .map(|order| DirectedTokenPair {
                    sell: C::canonical_token(order.sell_token, self.wrapped_native),
                    buy: C::canonical_token(order.buy_token, self.wrapped_native),
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
    #[instrument(skip_all)]
    pub fn compute_reference_scores(
        &self,
        ranking: &Ranking<C>,
        context: &AuctionContext<C>,
    ) -> HashMap<C::AccountId, C::Amount> {
        let mut reference_scores = HashMap::default();

        for ranked_solution in &ranking.ranked {
            let solver = ranked_solution.solver();

            if reference_scores.len() >= self.max_winners {
                return reference_scores;
            }
            if reference_scores.contains_key(&solver) {
                continue;
            }
            if !ranked_solution.is_winner() {
                continue;
            }

            // Compute score without this solver
            let solutions_without_solver: Vec<_> = ranking
                .ranked
                .iter()
                .filter(|s| s.solver() != solver)
                .collect();

            let winner_indices =
                self.pick_winners(solutions_without_solver.iter().copied(), context);

            let score = solutions_without_solver
                .iter()
                .enumerate()
                .filter(|(index, _)| winner_indices.contains(index))
                .map(|(_, solution)| solution.score())
                .reduce(|acc, score| acc.saturating_add(&score))
                .unwrap_or_else(C::Amount::zero);

            reference_scores.insert(solver, score);
        }

        reference_scores
    }
}

/// Computes the score of a single solution denominated in NATIVE token.
///
/// Same math the ranking path uses — intended for callers that already
/// know the winner (e.g. the fast-path handler) and only need per-solution
/// scores comparable with the regular-auction ones.
pub fn score<C: ChainTypes>(
    solution: &Solution<Unscored, C>,
    context: &AuctionContext<C>,
) -> Result<C::Amount> {
    Ok(sum_by_pair::<C>(&score_by_token_pair(solution, context)?))
}

/// Returns the total scores for each directed token pair of the solution.
/// E.g. if a solution contains 3 orders like:
///     sell A for B with a score of 10
///     sell A for B with a score of 5
///     sell B for C with a score of 5
/// it will return a map like:
///     (A, B) => 15
///     (B, C) => 5
fn score_by_token_pair<T, C: ChainTypes>(
    solution: &Solution<T, C>,
    context: &AuctionContext<C>,
) -> Result<HashMap<DirectedTokenPair<C>, C::Amount>> {
    let mut scores: HashMap<DirectedTokenPair<C>, C::Amount> = HashMap::default();

    for order in solution.orders() {
        if !context.contributes_to_score(&order.uid) {
            continue;
        }

        let score = compute_order_score(order, context)?;

        let token_pair = DirectedTokenPair {
            sell: order.sell_token,
            buy: order.buy_token,
        };

        let entry = scores.entry(token_pair).or_insert_with(C::Amount::zero);
        *entry = entry.saturating_add(&score);
    }

    Ok(scores)
}

/// Score defined as (surplus + protocol fees) first converted to buy
/// amounts and then converted to the native token.
///
/// Follows CIP-38 as the base of the score computation.
///
/// Denominated in NATIVE token.
fn compute_order_score<C: ChainTypes>(
    order: &Order<C>,
    context: &AuctionContext<C>,
) -> Result<C::Amount> {
    let native_price_buy = context
        .native_prices
        .get(&order.buy_token)
        .context("missing native price for buy token")?;

    // Calculate surplus in surplus token (buy token for sell orders, sell
    // token for buy orders)
    let surplus_in_surplus_token = {
        let user_surplus = surplus_over_limit(order)?;
        let fees = protocol_fees(order, context)?;

        user_surplus
            .try_add(fees)
            .context("overflow adding fees to surplus")?
    };

    let score_native = match order.side {
        // `surplus` of sell orders is already in buy tokens so we simply
        // convert it to the native token
        Side::Sell => C::value_in_native(*native_price_buy, surplus_in_surplus_token),
        Side::Buy => {
            // `surplus` of buy orders is in sell tokens. We start with
            // following formula: buy_amount / sell_amount
            // == buy_price / sell_price
            //
            // since `surplus` of buy orders is in sell tokens we convert to
            // buy amount via: buy_amount == (buy_price /
            // sell_price) * surplus
            //
            // to avoid loss of precision because we work with integers we
            // first multiply and then divide:
            // buy_amount = surplus * buy_price / sell_price
            let surplus_in_buy_tokens = surplus_in_surplus_token
                .try_widening_mul_div_floor(order.buy_amount, order.sell_amount)
                .context("converting surplus to buy tokens")?;

            // Afterwards we convert the buy token surplus to the native
            // token.
            C::value_in_native(*native_price_buy, surplus_in_buy_tokens)
        }
    };

    Ok(score_native)
}

/// Calculate total protocol fees for an order.
///
/// Returns the total fee in the surplus token.
fn protocol_fees<C: ChainTypes>(
    order: &Order<C>,
    context: &AuctionContext<C>,
) -> Result<C::Amount> {
    let policies = context
        .fee_policies
        .get(&order.uid)
        .map(|v| v.as_slice())
        .unwrap_or_default();
    protocol_fees_by_policy(order, policies)?
        .into_iter()
        .try_fold(C::Amount::zero(), |total, fee| total.try_add(fee))
        .context("overflow adding protocol fees")
}

/// Total solution score obtained by folding the per-token-pair scores
/// produced by [`score_by_token_pair`]. Used by both the ranking path and
/// the public [`score`] entry point so the two never diverge on how
/// directional scores are aggregated.
fn sum_by_pair<C: ChainTypes>(scores: &ScoreByDirection<C>) -> C::Amount {
    scores
        .values()
        .fold(C::Amount::zero(), |acc, s| acc.saturating_add(s))
}

/// Let's call a solution that only trades 1 directed token pair a baseline
/// solution. Returns the best baseline solution (highest score) for
/// each token pair if one exists.
fn compute_baseline_scores<C: ChainTypes>(
    scores_by_solution: &ScoresBySolution<C>,
) -> ScoreByDirection<C> {
    let mut baseline_scores = HashMap::default();

    for scores in scores_by_solution.values() {
        let Ok((token_pair, score)) = scores.iter().exactly_one() else {
            continue;
        };

        let current_best = baseline_scores
            .entry(token_pair.clone())
            .or_insert_with(C::Amount::zero);
        if score > current_best {
            *current_best = *score;
        }
    }

    baseline_scores
}

/// Result of partitioning solutions into fair and unfair.
struct PartitionedSolutions<C: ChainTypes> {
    /// Solutions that passed fairness checks (with scores).
    kept: Vec<Solution<Scored<C::Amount>, C>>,
    /// Solutions that were filtered out as unfair (with scores).
    discarded: Vec<Solution<Scored<C::Amount>, C>>,
}

/// Final ranking of all solutions.
#[derive(Debug)]
pub struct Ranking<C: ChainTypes = Evm> {
    /// Solutions that were filtered out as unfair (with scores and FilteredOut
    /// rank).
    pub filtered_out: Vec<Solution<Ranked<C::Amount>, C>>,
    /// Solutions that passed fairness checks, ordered by score (with
    /// Winner/NonWinner ranks).
    pub ranked: Vec<Solution<Ranked<C::Amount>, C>>,
}

impl<C: ChainTypes> Ranking<C> {
    /// All winning solutions.
    pub fn winners(&self) -> impl Iterator<Item = &Solution<Ranked<C::Amount>, C>> {
        self.ranked.iter().filter(|s| s.is_winner())
    }

    /// All non-winning solutions that weren't filtered out.
    pub fn non_winners(&self) -> impl Iterator<Item = &Solution<Ranked<C::Amount>, C>> {
        self.ranked.iter().filter(|s| !s.is_winner())
    }
}

/// Key to uniquely identify every solution.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SolutionKey<C: ChainTypes> {
    solver: C::AccountId,
    solution_id: u64,
}

/// Solutions annotated with their total score.
type ScoredSolutions<C> = Vec<Solution<Scored<<C as ChainTypes>::Amount>, C>>;

/// Scores of all trades in a solution aggregated by the directional
/// token pair. E.g. all trades (WETH -> USDC) are aggregated into
/// one value and all trades (USDC -> WETH) into another.
type ScoreByDirection<C> = HashMap<DirectedTokenPair<C>, <C as ChainTypes>::Amount>;

/// Mapping from solution to `DirectionalScores` for all solutions
/// of the auction.
type ScoresBySolution<C> = HashMap<SolutionKey<C>, ScoreByDirection<C>>;
