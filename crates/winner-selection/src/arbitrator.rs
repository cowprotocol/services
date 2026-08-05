//! Winner selection arbitrator.
//!
//! Implements the auction winner selection algorithm that picks the set of
//! solutions which maximize surplus while enforcing uniform directional
//! clearing prices. The algorithm is written once, generic over the chain's
//! type vocabulary ([`ChainTypes`]).

use {
    crate::{
        auction::AuctionContext,
        chain::{Amount, ChainTypes, MathError, MathResult},
        evm::Evm,
        primitives::{DirectedTokenPair, FeePolicy, Quote, Side},
        solution::{Order, RankType, Solution, Unscored},
        state::{Ranked, RankedItem, Scored, ScoredItem, UnscoredItem},
    },
    anyhow::{Context, Result},
    itertools::{Either, Itertools},
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
        let mut ranked = self.mark_winners(partitioned.kept);

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

        // Discard any solutions with a score of 0 as they are invalid
        solutions.retain(|solution| !solution.score().is_zero());

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

            // only keep solutions where each order execution is at least as good as
            // the baseline solution.
            // we only filter out unfair solutions with more than one token pair,
            // to avoid reference scores set to 0.
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
    ) -> Vec<Solution<Ranked<C::Amount>, C>> {
        let winner_indices = self.pick_winners(solutions.iter());

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
            match self.score_by_token_pair(&solution, context) {
                Ok(score) => {
                    let total_score = score
                        .values()
                        .fold(C::Amount::ZERO, |acc, s| acc.saturating_add(*s));
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

    /// Returns the total scores for each directed token pair of the solution.
    /// E.g. if a solution contains 3 orders like:
    ///     sell A for B with a score of 10
    ///     sell A for B with a score of 5
    ///     sell B for C with a score of 5
    /// it will return a map like:
    ///     (A, B) => 15
    ///     (B, C) => 5
    fn score_by_token_pair<T>(
        &self,
        solution: &Solution<T, C>,
        context: &AuctionContext<C>,
    ) -> Result<HashMap<DirectedTokenPair<C>, C::Amount>> {
        let mut scores: HashMap<DirectedTokenPair<C>, C::Amount> = HashMap::default();

        for order in solution.orders() {
            if !context.contributes_to_score(&order.uid) {
                continue;
            }

            let score = self.compute_order_score(order, context)?;

            let token_pair = DirectedTokenPair {
                sell: order.sell_token,
                buy: order.buy_token,
            };

            let entry = scores.entry(token_pair).or_default();
            *entry = entry.saturating_add(score);
        }

        Ok(scores)
    }

    /// Score defined as (surplus + protocol fees) first converted to buy
    /// amounts and then converted to the native token.
    ///
    /// Follows CIP-38 as the base of the score computation.
    ///
    /// Denominated in NATIVE token.
    fn compute_order_score(
        &self,
        order: &Order<C>,
        context: &AuctionContext<C>,
    ) -> Result<C::Amount> {
        let native_price_buy = context
            .native_prices
            .get(&order.buy_token)
            .context("missing native price for buy token")?;

        let custom_prices = self.calculate_custom_prices_from_executed(order);

        // Calculate surplus in surplus token (buy token for sell orders, sell token for
        // buy orders)
        let surplus_in_surplus_token = {
            let user_surplus = self.surplus_over_limit_price(order, &custom_prices)?;
            let fees = self.protocol_fees(order, context, &custom_prices)?;

            user_surplus
                .try_add(fees)
                .context("overflow adding fees to surplus")?
        };

        let score_native = match order.side {
            // `surplus` of sell orders is already in buy tokens so we simply convert it to the
            // native token
            Side::Sell => C::value_in_native(*native_price_buy, surplus_in_surplus_token),
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
                let surplus_in_buy_tokens = surplus_in_surplus_token
                    .try_widening_mul_div_floor(order.buy_amount, order.sell_amount)
                    .context("converting surplus to buy tokens")?;

                // Afterwards we convert the buy token surplus to the native token.
                C::value_in_native(*native_price_buy, surplus_in_buy_tokens)
            }
        };

        Ok(score_native)
    }

    /// Calculate total protocol fees for an order.
    ///
    /// Returns the total fee in the surplus token.
    fn protocol_fees(
        &self,
        order: &Order<C>,
        context: &AuctionContext<C>,
        base_prices: &ClearingPrices<C>,
    ) -> Result<C::Amount> {
        let policies = context
            .fee_policies
            .get(&order.uid)
            .map(|v| v.as_slice())
            .unwrap_or_default();

        let mut total_fee = C::Amount::ZERO;
        let mut current_prices = *base_prices;

        // Process policies in reverse order, updating custom prices as we go
        for (i, policy) in policies.iter().enumerate().rev() {
            let fee = self.protocol_fee(order, policy, &current_prices)?;

            total_fee = total_fee
                .try_add(fee)
                .context("overflow adding protocol fees")?;

            // Update custom prices for next iteration (except last iteration)
            if i != 0 {
                current_prices = self.calculate_custom_prices(order, total_fee, base_prices)?;
            }
        }

        Ok(total_fee)
    }

    /// Calculate a single protocol fee based on policy type.
    fn protocol_fee(
        &self,
        order: &Order<C>,
        policy: &FeePolicy<C>,
        custom_prices: &ClearingPrices<C>,
    ) -> MathResult<C::Amount> {
        match policy {
            FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => {
                let surplus = self.surplus_over_limit_price(order, custom_prices)?;
                let surplus_fee = self.surplus_fee(surplus, *factor)?;
                let volume_fee = self.volume_fee(order, custom_prices, *max_volume_factor)?;
                Ok(surplus_fee.min(volume_fee))
            }
            FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => {
                let price_improvement =
                    self.price_improvement_over_quote(order, custom_prices, quote)?;
                let surplus_fee = self.surplus_fee(price_improvement, *factor)?;
                let volume_fee = self.volume_fee(order, custom_prices, *max_volume_factor)?;
                Ok(surplus_fee.min(volume_fee))
            }
            FeePolicy::Volume { factor } => self.volume_fee(order, custom_prices, *factor),
        }
    }

    /// Calculate surplus over limit price using custom clearing prices.
    fn surplus_over_limit_price(
        &self,
        order: &Order<C>,
        prices: &ClearingPrices<C>,
    ) -> MathResult<C::Amount> {
        self.surplus_over(
            order,
            prices,
            PriceLimits {
                sell: order.sell_amount,
                buy: order.buy_amount,
            },
        )
    }

    /// Calculate surplus over arbitrary price limits.
    fn surplus_over(
        &self,
        order: &Order<C>,
        prices: &ClearingPrices<C>,
        limits: PriceLimits<C>,
    ) -> MathResult<C::Amount> {
        let executed = match order.side {
            Side::Buy => order.executed_buy,
            Side::Sell => order.executed_sell,
        };

        match order.side {
            Side::Buy => {
                // Scale limit sell to support partially fillable orders
                let limit_sell = limits.sell.try_mul_div_floor(executed, limits.buy)?;

                let sold = executed.try_mul_div_floor(prices.buy, prices.sell)?;

                limit_sell.try_sub(sold)
            }
            Side::Sell => {
                // Scale limit buy to support partially fillable orders (ceiling division)
                let limit_buy = executed.try_mul_div_ceil(limits.buy, limits.sell)?;

                let bought = executed.try_mul_div_ceil(prices.sell, prices.buy)?;

                bought.try_sub(limit_buy)
            }
        }
    }

    /// Calculate price improvement over quote.
    ///
    /// Returns 0 if there's no improvement (instead of error).
    fn price_improvement_over_quote(
        &self,
        order: &Order<C>,
        prices: &ClearingPrices<C>,
        quote: &Quote<C>,
    ) -> MathResult<C::Amount> {
        let adjusted_quote = self.adjust_quote_to_order_limits(order, quote)?;
        match self.surplus_over(order, prices, adjusted_quote) {
            Ok(surplus) => Ok(surplus),
            Err(MathError::Negative) => Ok(C::Amount::ZERO),
            Err(err) => Err(err),
        }
    }

    /// Adjust quote amounts to be comparable with order limits.
    fn adjust_quote_to_order_limits(
        &self,
        order: &Order<C>,
        quote: &Quote<C>,
    ) -> MathResult<PriceLimits<C>> {
        match order.side {
            Side::Sell => {
                // Quote buy amount after fees
                let quote_buy_amount = quote.buy_amount.try_sub(
                    quote
                        .fee
                        .try_mul_div_floor(quote.buy_amount, quote.sell_amount)?,
                )?;

                // Scale to order's sell amount
                let scaled_buy_amount =
                    quote_buy_amount.try_mul_div_floor(order.sell_amount, quote.sell_amount)?;

                // Use max to handle out-of-market orders
                let buy_amount = order.buy_amount.max(scaled_buy_amount);

                Ok(PriceLimits {
                    sell: order.sell_amount,
                    buy: buy_amount,
                })
            }
            Side::Buy => {
                // Quote sell amount including fees
                let quote_sell_amount = quote.sell_amount.try_add(quote.fee)?;

                // Scale to order's buy amount
                let scaled_sell_amount =
                    quote_sell_amount.try_mul_div_floor(order.buy_amount, quote.buy_amount)?;

                // Use min to handle out-of-market orders
                let sell_amount = order.sell_amount.min(scaled_sell_amount);

                Ok(PriceLimits {
                    sell: sell_amount,
                    buy: order.buy_amount,
                })
            }
        }
    }

    /// Calculate surplus fee as a cut of surplus.
    ///
    /// Uses adjusted factor: fee = surplus * factor / (1 - factor)
    fn surplus_fee(&self, surplus: C::Amount, factor: f64) -> MathResult<C::Amount> {
        // Surplus fee is specified as a `factor` from raw surplus (before fee).
        // Since we work with trades that already have the protocol fee applied,
        // we need to calculate the protocol fee using an adjusted factor.
        //
        // fee = surplus_before_fee * factor
        // surplus_after_fee = surplus_before_fee - fee
        // fee = surplus_after_fee * factor / (1 - factor)
        surplus.try_mul_f64(factor / (1.0 - factor))
    }

    /// Calculate volume fee as a cut of trade volume.
    fn volume_fee(
        &self,
        order: &Order<C>,
        prices: &ClearingPrices<C>,
        factor: f64,
    ) -> MathResult<C::Amount> {
        // Volume fee is specified as a factor from raw volume (before fee).
        // We need to calculate using an adjusted factor based on order side.
        //
        // Sell: fee = traded_buy_amount * factor / (1 - factor)
        // Buy:  fee = traded_sell_amount * factor / (1 + factor)

        let executed_in_surplus_token = match order.side {
            Side::Sell => self.buy_amount(order, prices)?,
            Side::Buy => self.sell_amount(order, prices)?,
        };

        let adjusted_factor = match order.side {
            Side::Sell => factor / (1.0 - factor),
            Side::Buy => factor / (1.0 + factor),
        };

        executed_in_surplus_token.try_mul_f64(adjusted_factor)
    }

    /// Calculate custom clearing prices from executed amounts.
    ///
    /// Custom prices are derived from what was actually executed.
    fn calculate_custom_prices_from_executed(&self, order: &Order<C>) -> ClearingPrices<C> {
        ClearingPrices {
            sell: order.executed_buy,
            buy: order.executed_sell,
        }
    }

    /// Calculate custom clearing prices excluding protocol fees.
    ///
    /// This adjusts prices to reflect the trade without the accumulated fees.
    fn calculate_custom_prices(
        &self,
        order: &Order<C>,
        protocol_fee: C::Amount,
        prices: &ClearingPrices<C>,
    ) -> MathResult<ClearingPrices<C>> {
        let sell_amount = self.sell_amount(order, prices)?;
        let buy_amount = self.buy_amount(order, prices)?;

        Ok(ClearingPrices {
            sell: match order.side {
                Side::Sell => buy_amount.try_add(protocol_fee)?,
                Side::Buy => buy_amount,
            },
            buy: match order.side {
                Side::Sell => sell_amount,
                Side::Buy => sell_amount.try_sub(protocol_fee)?,
            },
        })
    }

    /// Calculate effective sell amount (what left user's wallet).
    fn sell_amount(&self, order: &Order<C>, prices: &ClearingPrices<C>) -> MathResult<C::Amount> {
        match order.side {
            Side::Sell => Ok(order.executed_sell),
            Side::Buy => order
                .executed_buy
                .try_mul_div_floor(prices.buy, prices.sell),
        }
    }

    /// Calculate effective buy amount (what user received).
    fn buy_amount(&self, order: &Order<C>, prices: &ClearingPrices<C>) -> MathResult<C::Amount> {
        match order.side {
            Side::Sell => order
                .executed_sell
                .try_mul_div_ceil(prices.sell, prices.buy),
            Side::Buy => Ok(order.executed_buy),
        }
    }

    /// Returns indices of winning solutions.
    /// Assumes that `solutions` is sorted by score descendingly.
    /// This logic was moved into a helper function to avoid a ton of `.clone()`
    /// operations in `compute_reference_scores()`.
    fn pick_winners<'a, T: 'a>(
        &self,
        solutions: impl Iterator<Item = &'a Solution<T, C>>,
    ) -> HashSet<usize> {
        // Winners are selected one by one, starting from the best solution,
        // until `max_winners` are selected. A solution can only
        // win if none of the (sell_token, buy_token) pairs of the executed
        // orders have been covered by any previously selected winning solution.
        // In other words this enforces a uniform **directional** clearing price.
        let mut already_swapped_token_pairs = HashSet::new();
        let mut winners = HashSet::default();

        for (index, solution) in solutions.enumerate() {
            if winners.len() >= self.max_winners {
                return winners;
            }

            let swapped_token_pairs: HashSet<DirectedTokenPair<C>> = solution
                .orders()
                .iter()
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

            let winner_indices = self.pick_winners(solutions_without_solver.iter().copied());

            let score = solutions_without_solver
                .iter()
                .enumerate()
                .filter(|(index, _)| winner_indices.contains(index))
                .map(|(_, solution)| solution.score())
                .reduce(|acc, score| acc.saturating_add(score))
                .unwrap_or_default();

            reference_scores.insert(solver, score);
        }

        reference_scores
    }
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

        let current_best = baseline_scores.entry(token_pair.clone()).or_default();
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

/// Clearing prices for a trade.
///
/// These can be either uniform (same for all orders) or custom (adjusted for
/// protocol fees on a per-order basis).
#[derive(Debug, Clone, Copy)]
struct ClearingPrices<C: ChainTypes> {
    /// Price of sell token in terms of buy token.
    sell: C::Amount,
    /// Price of buy token in terms of sell token.
    buy: C::Amount,
}

/// Price limits for an order or quote.
#[derive(Debug, Clone, Copy)]
struct PriceLimits<C: ChainTypes> {
    /// Maximum sell amount.
    sell: C::Amount,
    /// Minimum buy amount.
    buy: C::Amount,
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
