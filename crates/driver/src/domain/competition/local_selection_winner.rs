use {
    alloy::{consensus::private::alloy_primitives, signers::Either},
    alloy_primitives::keccak256,
    anyhow::Context,
    derive_more::{Display, From, Into},
    itertools::Itertools,
    num::Saturating,
    primitive_types::{H160, U256},
    std::collections::{HashMap, HashSet},
};

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    From,
    Into,
    Display,
    Default,
    derive_more::Add,
    derive_more::Sub,
)]
pub struct Ether(pub U256);

impl num::Saturating for Ether {
    fn saturating_add(self, v: Self) -> Self {
        Self(self.0.saturating_add(v.0))
    }

    fn saturating_sub(self, v: Self) -> Self {
        Self(self.0.saturating_sub(v.0))
    }
}

impl num::CheckedSub for Ether {
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        self.0.checked_sub(v.0).map(Ether)
    }
}

impl num::Zero for Ether {
    fn zero() -> Self {
        Self(U256::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl std::iter::Sum for Ether {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(num::Zero::zero(), num::Saturating::saturating_add)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Display,
    Default,
    derive_more::Add,
    derive_more::Sub,
    Eq,
    Ord,
)]
pub struct Score(Ether);

impl Score {
    pub fn try_new(score: Ether) -> Result<Self, ZeroScore> {
        if score.0.is_zero() {
            Err(ZeroScore)
        } else {
            Ok(Self(score))
        }
    }

    pub fn get(&self) -> &Ether {
        &self.0
    }

    pub fn saturating_add_assign(&mut self, other: Self) {
        self.0 = self.0.saturating_add(other.0);
    }
}

impl num::Saturating for Score {
    fn saturating_add(self, v: Self) -> Self {
        Self(self.0.saturating_add(v.0))
    }

    fn saturating_sub(self, v: Self) -> Self {
        Self(self.0.saturating_sub(v.0))
    }
}

impl num::CheckedSub for Score {
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        self.0.checked_sub(&v.0).map(Score)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the solver proposed a 0-score solution")]
pub struct ZeroScore;

#[derive(Debug, thiserror::Error)]
#[error("price cannot be zero")]
pub struct InvalidPrice;

#[derive(Debug, thiserror::Error)]
pub enum SolutionError {
    #[error(transparent)]
    ZeroScore(#[from] ZeroScore),
    #[error(transparent)]
    InvalidPrice(#[from] InvalidPrice),
    #[error("the solver got deny listed")]
    SolverDenyListed,
}

pub struct ParticipantDetached<State = Ranked> {
    solution: crate::infra::api::routes::solve::dto::solve_response::Solution,
    submission_address: crate::domain::eth::H160,
    state: State,
    computed_score: Option<Score>,
}

pub struct Unranked;
pub enum Ranked {
    Winner,
    NonWinner,
    FilteredOut,
}

impl<T> ParticipantDetached<T> {
    pub fn solution(&self) -> &crate::infra::api::routes::solve::dto::solve_response::Solution {
        &self.solution
    }

    pub fn set_computed_score(&mut self, score: Score) {
        self.computed_score = Some(score);
    }

    pub fn computed_score(&self) -> Option<&Score> {
        self.computed_score.as_ref()
    }

    pub fn submission_address(&self) -> &crate::domain::eth::H160 {
        &self.submission_address
    }
}

impl ParticipantDetached<Unranked> {
    pub fn new(solution: crate::infra::api::routes::solve::dto::solve_response::Solution) -> Self {
        let address = solution.submission_address;
        Self {
            solution,
            submission_address: address,
            state: Unranked,
            computed_score: None,
        }
    }

    pub fn rank(self, rank: Ranked) -> ParticipantDetached<Ranked> {
        ParticipantDetached::<Ranked> {
            state: rank,
            submission_address: self.submission_address,
            solution: self.solution,
            computed_score: self.computed_score,
        }
    }
}

impl ParticipantDetached<Ranked> {
    pub fn is_winner(&self) -> bool {
        matches!(self.state, Ranked::Winner)
    }

    pub fn filtered_out(&self) -> bool {
        matches!(self.state, Ranked::FilteredOut)
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct DirectedTokenPair {
    sell: crate::domain::eth::H160,
    buy: crate::domain::eth::H160,
}

/// Key to uniquely identify every solution.
#[derive(PartialEq, Eq, std::hash::Hash)]
struct SolutionKey {
    driver: crate::domain::eth::H160,
    solution_id: u64,
}

type ScoreByDirection = HashMap<DirectedTokenPair, Score>;

/// Mapping from solution to `DirectionalScores` for all solutions
/// of the auction.
type ScoresBySolution = HashMap<SolutionKey, ScoreByDirection>;

pub struct Ranking {
    /// Solutions that were discarded because they were malformed
    /// in some way or deemed unfair by the selection mechanism.
    filtered_out: Vec<ParticipantDetached<Ranked>>,
    /// Final ranking of the solutions that passed the fairness
    /// check. Winners come before non-winners and higher total
    /// scores come before lower scores.
    ranked: Vec<ParticipantDetached<Ranked>>,
}

impl Ranking {
    /// All solutions including the ones that got filtered out.
    pub fn all(&self) -> impl Iterator<Item = &ParticipantDetached<Ranked>> {
        self.ranked.iter().chain(&self.filtered_out)
    }

    /// Enumerates all solutions. The index is used as solution UID.
    pub fn enumerated(&self) -> impl Iterator<Item = (usize, &ParticipantDetached<Ranked>)> {
        self.all().enumerate()
    }

    /// All solutions that won the right to get executed.
    pub fn winners(&self) -> impl Iterator<Item = &ParticipantDetached<Ranked>> {
        self.ranked.iter().filter(|p| p.is_winner())
    }

    /// All solutions that were not filtered out but also did not win.
    pub fn non_winners(&self) -> impl Iterator<Item = &ParticipantDetached<Ranked>> {
        self.ranked.iter().filter(|p| !p.is_winner())
    }

    /// All solutions that passed the filtering step.
    pub fn ranked(&self) -> impl Iterator<Item = &ParticipantDetached<Ranked>> {
        self.ranked.iter()
    }
}

struct PartitionedSolutions {
    kept: Vec<ParticipantDetached<Unranked>>,
    discarded: Vec<ParticipantDetached<Unranked>>,
}

pub struct OptimizedAuction {
    /// Fee policies for **all** orders that were in the original auction.
    pub fee_policies: HashMap<autopilot::domain::OrderUid, Vec<autopilot::domain::fee::Policy>>,
    surplus_capturing_jit_order_owners: HashSet<autopilot::domain::eth::Address>,
    pub native_prices: autopilot::domain::auction::Prices,
}

impl OptimizedAuction {
    /// Returns whether an order is allowed to capture surplus and
    /// therefore contributes to the total score of a solution.
    pub fn contributes_to_score(&self, uid: &autopilot::domain::auction::order::OrderUid) -> bool {
        self.fee_policies.contains_key(uid)
            || self
                .surplus_capturing_jit_order_owners
                .contains(&uid.owner())
    }
}

impl From<&crate::domain::competition::Auction> for OptimizedAuction {
    fn from(original: &crate::domain::competition::Auction) -> Self {
        let mut fee_policies = HashMap::new();

        for order in &original.orders {
            let uid_bytes: [u8; 56] = order.uid.0.0;
            let order_uid = autopilot::domain::OrderUid(uid_bytes);

            let policies: Vec<autopilot::domain::fee::Policy> = order
                .protocol_fees
                .clone()
                .into_iter()
                .map(|fee_policy| match fee_policy {
                    crate::domain::competition::order::fees::FeePolicy::Surplus {
                        factor,
                        max_volume_factor,
                    } => autopilot::domain::fee::Policy::Surplus {
                        factor: autopilot::domain::fee::FeeFactor::try_from(factor)
                            .expect("invalid fee"),
                        max_volume_factor: autopilot::domain::fee::FeeFactor::try_from(
                            max_volume_factor,
                        )
                        .expect("invalid max_volume_factor"),
                    },
                    crate::domain::competition::order::fees::FeePolicy::PriceImprovement {
                        factor,
                        max_volume_factor,
                        quote,
                    } => {
                        let q = autopilot::domain::fee::Quote {
                            sell_amount: quote.sell.amount.into(),
                            buy_amount: quote.buy.amount.into(),
                            fee: quote.fee.amount.into(),
                            solver: alloy_primitives::Address::from(
                                quote.solver.0.to_fixed_bytes(),
                            ),
                        };
                        autopilot::domain::fee::Policy::PriceImprovement {
                            factor: autopilot::domain::fee::FeeFactor::try_from(factor)
                                .expect("invalid fee"),
                            max_volume_factor: autopilot::domain::fee::FeeFactor::try_from(
                                max_volume_factor,
                            )
                            .expect("invalid max_volume_factor"),
                            quote: q,
                        }
                    }

                    crate::domain::competition::order::fees::FeePolicy::Volume { factor } => {
                        autopilot::domain::fee::Policy::Volume {
                            factor: autopilot::domain::fee::FeeFactor::try_from(factor)
                                .expect("invalid fee"),
                        }
                    }
                })
                .collect();

            fee_policies.insert(order_uid, policies);
        }

        let surplus_capturing_jit_order_owners: HashSet<autopilot::domain::eth::Address> = original
            .surplus_capturing_jit_order_owners
            .iter()
            .map(|addr| autopilot::domain::eth::Address(addr.0))
            .collect();

        let native_prices: HashMap<_, _> = original
            .tokens
            .iter_keys_values()
            .filter_map(|(token_addr, token)| {
                token.price.map(|p| {
                    let addr = autopilot::domain::eth::TokenAddress(token_addr.0.0);
                    let price = autopilot::domain::auction::Price::try_new(
                        autopilot::domain::eth::Ether(p.0.0),
                    )
                    .expect("invalid price");
                    (addr, price)
                })
            })
            .collect();

        Self {
            fee_policies,
            surplus_capturing_jit_order_owners,
            native_prices,
        }
    }
}

#[derive(Clone)]
pub struct LocalArbitrator {
    pub max_winners: usize,
    pub weth: autopilot::domain::eth::WrappedNativeToken,
}

impl LocalArbitrator {
    /// Runs the entire auction mechanism on the passed in solutions.
    pub fn arbitrate(
        &self,
        participants: Vec<ParticipantDetached<Unranked>>,
        auction: &crate::domain::competition::Auction,
    ) -> Ranking {
        let mut participants = participants;
        participants.sort_by(|a, b| {
            let ha = hash_solution(a.solution());
            let hb = hash_solution(b.solution());
            ha.cmp(&hb)
        });

        let partitioned = self.partition_unfair_solutions(participants, auction);
        let filtered_out = partitioned
            .discarded
            .into_iter()
            .map(|participant| participant.rank(Ranked::FilteredOut))
            .collect();

        let mut ranked = self.mark_winners(partitioned.kept);
        ranked.sort_by_key(|participant| {
            (
                // winners before non-winners
                std::cmp::Reverse(participant.is_winner()),
                // high score before low score
                std::cmp::Reverse(participant.computed_score().cloned()),
            )
        });
        Ranking {
            filtered_out,
            ranked,
        }
    }

    /// Removes unfair solutions from the set of all solutions.
    fn partition_unfair_solutions(
        &self,
        mut participants: Vec<ParticipantDetached<Unranked>>,
        auction: &crate::domain::competition::Auction,
    ) -> PartitionedSolutions {
        // Discard all solutions where we can't compute the aggregate scores
        // accurately because the fairness guarantees heavily rely on them.
        let scores_by_solution = compute_scores_by_solution(&mut participants, auction);
        participants.sort_by_key(|participant| {
            std::cmp::Reverse(
                // we use the computed score to not trust the score provided by solvers
                participant
                    .computed_score()
                    .expect("every remaining participant has a computed score")
                    .get()
                    .0,
            )
        });
        let baseline_scores = compute_baseline_scores(&scores_by_solution);
        let (fair, unfair) = participants.into_iter().partition_map(|p| {
            let aggregated_scores = scores_by_solution
                .get(&SolutionKey {
                    driver: p.submission_address,
                    solution_id: p.solution().solution_id,
                })
                .expect("every remaining participant has an entry");
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
                Either::Left(p)
            } else {
                Either::Right(p)
            }
        });
        PartitionedSolutions {
            kept: fair,
            discarded: unfair,
        }
    }

    /// Picks winners and sorts all solutions where winners come before
    /// non-winners and higher scores come before lower scores.
    fn mark_winners(
        &self,
        participants: Vec<ParticipantDetached<Unranked>>,
    ) -> Vec<ParticipantDetached<Ranked>> {
        // Pick winners based on their solutions
        let winner_indexes = self.pick_winners(participants.iter().map(|p| p.solution()));

        participants
            .into_iter()
            .enumerate()
            .map(|(index, participant)| {
                let rank = if winner_indexes.contains(&index) {
                    Ranked::Winner
                } else {
                    Ranked::NonWinner
                };
                participant.rank(rank)
            })
            .collect()
    }

    /// Returns indices of winning solutions.
    /// Assumes that `solutions` is sorted by score descendingly.
    /// This logic was moved into a helper function to avoid a ton of `.clone()`
    /// operations in `compute_reference_scores()`.
    fn pick_winners<'a>(
        &self,
        solutions: impl Iterator<
            Item = &'a crate::infra::api::routes::solve::dto::solve_response::Solution,
        >,
    ) -> HashSet<usize> {
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
                .orders
                .values()
                .map(|order| DirectedTokenPair {
                    sell: as_erc20(H160::from(order.sell_token.0), self.weth),
                    buy: as_erc20(H160::from(order.buy_token.0), self.weth),
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

pub fn as_erc20(
    token: H160,
    wrapped_native_token: autopilot::domain::eth::WrappedNativeToken,
) -> H160 {
    if token == autopilot::domain::eth::NATIVE_TOKEN.0 {
        let wrapped: autopilot::domain::eth::TokenAddress = wrapped_native_token.into();
        wrapped.0
    } else {
        token
    }
}

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
    participants: &mut Vec<ParticipantDetached<Unranked>>,
    auction: &crate::domain::competition::Auction,
) -> ScoresBySolution {
    let auction = OptimizedAuction::from(auction);
    let mut scores = HashMap::default();

    participants.retain_mut(|p| match score_by_token_pair(p.solution(), &auction) {
        Ok(score) => {
            let total_score = score
                .values()
                .fold(Score::default(), |acc, score| acc.saturating_add(*score));
            scores.insert(
                SolutionKey {
                    driver: p.submission_address,
                    solution_id: p.solution().solution_id,
                },
                score,
            );
            p.set_computed_score(total_score);
            true
        }
        Err(err) => {
            tracing::warn!(
                driver = p.submission_address.to_string(),
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
fn score_by_token_pair(
    solution: &crate::infra::api::routes::solve::dto::solve_response::Solution,
    auction: &OptimizedAuction,
) -> anyhow::Result<ScoreByDirection> {
    let mut scores: HashMap<DirectedTokenPair, Score> = HashMap::default();
    for (uid, order) in &solution.orders {
        if !auction.contributes_to_score(&autopilot::domain::OrderUid(*uid)) {
            continue;
        }

        let uniform_sell_price = solution
            .clearing_prices
            .get(&order.sell_token)
            .context("no uniform clearing price for sell token")?;
        let uniform_buy_price = solution
            .clearing_prices
            .get(&order.buy_token)
            .context("no uniform clearing price for buy token")?;

        let trade = autopilot::domain::settlement::math::Trade {
            uid: autopilot::domain::OrderUid(*uid),
            side: match order.side {
                crate::infra::api::routes::solve::dto::solve_response::Side::Buy => {
                    autopilot::domain::auction::order::Side::Buy
                }
                crate::infra::api::routes::solve::dto::solve_response::Side::Sell => {
                    autopilot::domain::auction::order::Side::Sell
                }
            },
            sell: autopilot::domain::eth::Asset {
                token: autopilot::domain::eth::TokenAddress(order.sell_token),
                amount: autopilot::domain::eth::TokenAmount(order.limit_sell),
            },
            buy: autopilot::domain::eth::Asset {
                token: autopilot::domain::eth::TokenAddress(order.buy_token),
                amount: autopilot::domain::eth::TokenAmount(order.limit_buy),
            },
            executed: autopilot::domain::auction::order::TargetAmount(match order.side {
                crate::infra::api::routes::solve::dto::solve_response::Side::Sell => {
                    order.executed_sell
                }
                crate::infra::api::routes::solve::dto::solve_response::Side::Buy => {
                    order.executed_buy
                }
            }),
            prices: autopilot::domain::settlement::transaction::Prices {
                uniform: autopilot::domain::settlement::transaction::ClearingPrices {
                    sell: *uniform_sell_price,
                    buy: *uniform_buy_price,
                },
                custom: autopilot::domain::settlement::transaction::ClearingPrices {
                    sell: order.executed_buy,
                    buy: order.executed_sell,
                },
            },
        };
        let score = trade
            .score(&auction.fee_policies, &auction.native_prices)
            .context("failed to compute score")?;

        tracing::info!(
            uniform_sell_price = ?uniform_sell_price,
            uniform_buy_price = ?uniform_buy_price,
            trade = ?trade,
            fee_policies = ?auction.fee_policies,
            native_prices = ?auction.native_prices,
            solver = %solution.submission_address.to_string(),
            score = ?score,
            "[pod] local score_by_token_pair"
        );

        let token_pair = DirectedTokenPair {
            sell: trade.sell.token.0,
            buy: trade.buy.token.0,
        };

        scores
            .entry(token_pair)
            .or_default()
            .saturating_add_assign(Score(Ether(score.0)));
    }
    Ok(scores)
}

fn u64_to_be_bytes(x: u64) -> [u8; 8] {
    x.to_be_bytes()
}

fn u256_to_be_bytes(x: &crate::domain::eth::U256) -> [u8; 32] {
    let mut out = [0u8; 32];
    // adjust this depending on your U256 implementation
    x.to_big_endian(&mut out);
    out
}

fn h160_to_bytes(x: &crate::domain::eth::H160) -> [u8; 20] {
    // if H160 is a tuple struct: pub struct H160(pub [u8; 20]);
    x.0
}

fn side_to_byte(side: &crate::infra::api::routes::solve::dto::solve_response::Side) -> u8 {
    match side {
        crate::infra::api::routes::solve::dto::solve_response::Side::Buy => 0u8,
        crate::infra::api::routes::solve::dto::solve_response::Side::Sell => 1u8,
    }
}

fn encode_traded_order(
    buf: &mut Vec<u8>,
    order: &crate::infra::api::routes::solve::dto::solve_response::TradedOrder,
) {
    buf.push(side_to_byte(&order.side));

    buf.extend_from_slice(&h160_to_bytes(&order.sell_token));
    buf.extend_from_slice(&u256_to_be_bytes(&order.limit_sell));

    buf.extend_from_slice(&h160_to_bytes(&order.buy_token));
    buf.extend_from_slice(&u256_to_be_bytes(&order.limit_buy));

    buf.extend_from_slice(&u256_to_be_bytes(&order.executed_sell));
    buf.extend_from_slice(&u256_to_be_bytes(&order.executed_buy));
}

pub fn hash_solution(
    solution: &crate::infra::api::routes::solve::dto::solve_response::Solution,
) -> [u8; 32] {
    let mut data = Vec::new();

    data.extend_from_slice(&u64_to_be_bytes(solution.solution_id));
    data.extend_from_slice(&u256_to_be_bytes(&solution.score));
    data.extend_from_slice(&h160_to_bytes(&solution.submission_address));

    let mut orders: Vec<(
        &crate::infra::api::routes::solve::dto::solve_response::OrderId,
        &crate::infra::api::routes::solve::dto::solve_response::TradedOrder,
    )> = solution.orders.iter().collect();
    orders.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    data.extend_from_slice(&u64_to_be_bytes(orders.len() as u64));

    for (order_id, traded_order) in orders {
        // OrderId is [u8; UID_LEN]
        data.extend_from_slice(order_id);
        encode_traded_order(&mut data, traded_order);
    }

    let mut prices: Vec<(&crate::domain::eth::H160, &crate::domain::eth::U256)> =
        solution.clearing_prices.iter().collect();
    prices.sort_by(|(a, _), (b, _)| h160_to_bytes(a).cmp(&h160_to_bytes(b)));

    data.extend_from_slice(&u64_to_be_bytes(prices.len() as u64));

    for (token, price) in prices {
        data.extend_from_slice(&h160_to_bytes(token));
        data.extend_from_slice(&u256_to_be_bytes(price));
    }

    keccak256(&data).into()
}
