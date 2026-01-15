use {
    crate::domain::{competition::order::FeePolicy, eth::Address},
    alloy::primitives::keccak256,
    autopilot::domain::{
        competition::Score,
        eth::{self, WrappedNativeToken},
    },
    std::collections::HashMap,
    winner_selection::{
        self as winsel,
        OrderUid,
        RankType,
        Unscored,
        state::{self, HasState, RankedItem, ScoredItem, UnscoredItem},
    },
};

#[derive(Clone)]
pub struct SolverArbitrator(winsel::Arbitrator);

/// Implements auction arbitration in 3 phases:
/// 1. filter unfair solutions
/// 2. mark winners
/// 3. compute reference scores
///
/// The functions assume the `Arbitrator` is the only one
/// changing the ordering or the `bids`.
impl SolverArbitrator {
    pub fn new(max_winners: usize, wrapped_native_token: WrappedNativeToken) -> Self {
        Self(winsel::Arbitrator {
            max_winners,
            weth: wrapped_native_token.into(),
        })
    }

    /// Runs the entire auction mechanism on the passed in solutions.
    pub fn arbitrate(
        &self,
        bids: Vec<Bid<Unscored>>,
        auction: &crate::domain::competition::Auction,
    ) -> Ranking {
        let mut bids = bids;
        bids.sort_by_cached_key(|b| hash_solution(b.solution()));

        let context = auction.into();
        let mut bid_by_key = HashMap::with_capacity(bids.len());
        let mut solutions = Vec::with_capacity(bids.len());

        for bid in bids {
            let key = SolutionKey::from(bid.solution());
            let solution = bid.solution().into();
            bid_by_key.insert(key, bid);
            solutions.push(solution);
        }

        let ws_ranking = self.0.arbitrate(solutions, &context);

        // Compute reference scores while we still have ws_ranking
        let reference_scores: HashMap<Address, Score> = self
            .0
            .compute_reference_scores(&ws_ranking)
            .into_iter()
            .map(|(solver, score)| (solver, Score(eth::Ether(score))))
            .collect();

        let mut filtered_out = Vec::with_capacity(ws_ranking.filtered_out.len());
        for ws_solution in ws_ranking.filtered_out {
            let key = SolutionKey::from(&ws_solution);
            let bid = bid_by_key
                .remove(&key)
                .expect("every ranked solution has a matching bid");
            let score = ws_solution.score();
            filtered_out.push(
                bid.with_score(Score(eth::Ether(score)))
                    .with_rank(RankType::FilteredOut),
            );
        }

        let mut ranked = Vec::with_capacity(ws_ranking.ranked.len());
        for ranked_solution in ws_ranking.ranked {
            let key = SolutionKey::from(&ranked_solution);
            let bid = bid_by_key
                .remove(&key)
                .expect("every ranked solution has a matching bid");
            let score = ranked_solution.score();
            ranked.push(
                bid.with_score(Score(eth::Ether(score)))
                    .with_rank(ranked_solution.state().rank_type),
            );
        }

        Ranking {
            filtered_out,
            ranked,
            reference_scores,
        }
    }
}

impl From<&crate::domain::competition::Auction> for winsel::AuctionContext {
    fn from(auction: &crate::domain::competition::Auction) -> Self {
        Self {
            fee_policies: auction
                .orders
                .iter()
                .map(|order| {
                    let uid = winsel::OrderUid(order.uid.0.0);
                    let policies = order
                        .protocol_fees
                        .iter()
                        //.copied()
                        .map(winsel::primitives::FeePolicy::from)
                        .collect();
                    (uid, policies)
                })
                .collect(),
            surplus_capturing_jit_order_owners: auction
                .surplus_capturing_jit_order_owners
                .iter()
                .copied()
                .collect(),
            native_prices: auction
                .tokens
                .iter_keys_values()
                .map(|(token, price)| (token.0.0, price.price.unwrap().0.0))
                .collect(),
        }
    }
}

impl From<&crate::domain::competition::order::fees::FeePolicy> for winsel::primitives::FeePolicy {
    fn from(policy: &crate::domain::competition::order::fees::FeePolicy) -> Self {
        match policy {
            FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => Self::Surplus {
                factor: *factor,
                max_volume_factor: *max_volume_factor,
            },
            FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self::PriceImprovement {
                factor: *factor,
                max_volume_factor: *max_volume_factor,
                quote: winsel::primitives::Quote {
                    sell_amount: quote.sell.amount.0,
                    buy_amount: quote.buy.amount.0,
                    fee: quote.fee.amount.0,
                    solver: quote.solver,
                },
            },
            FeePolicy::Volume { factor } => Self::Volume { factor: *factor },
        }
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct SolutionKey {
    solver: eth::Address,
    solution_id: u64,
}

impl From<&crate::infra::api::routes::solve::dto::solve_response::Solution> for SolutionKey {
    fn from(solution: &crate::infra::api::routes::solve::dto::solve_response::Solution) -> Self {
        Self {
            solver: solution.submission_address,
            solution_id: solution.solution_id,
        }
    }
}

impl<S> From<&winsel::Solution<S>> for SolutionKey {
    fn from(solution: &winsel::Solution<S>) -> Self {
        Self {
            solver: solution.solver(),
            solution_id: solution.id(),
        }
    }
}

impl From<&crate::infra::api::routes::solve::dto::solve_response::Solution>
    for winsel::Solution<winsel::Unscored>
{
    fn from(solution: &crate::infra::api::routes::solve::dto::solve_response::Solution) -> Self {
        Self::new(
            solution.solution_id,
            solution.submission_address,
            solution
                .orders
                .iter()
                .map(|(uid, order)| winsel::Order {
                    uid: OrderUid(*uid),
                    sell_token: order.sell_token,
                    buy_token: order.buy_token,
                    sell_amount: order.limit_sell,
                    buy_amount: order.limit_buy,
                    executed_sell: order.executed_sell,
                    executed_buy: order.executed_buy,
                    side: match order.side {
                        crate::infra::api::routes::solve::dto::solve_response::Side::Buy => {
                            winsel::Side::Buy
                        }
                        crate::infra::api::routes::solve::dto::solve_response::Side::Sell => {
                            winsel::Side::Sell
                        }
                    },
                })
                .collect(),
            solution.clearing_prices.clone(),
        )
    }
}

pub type Scored = winsel::state::Scored<Score>;
pub type Ranked = winsel::state::Ranked<Score>;

/// A solver's auction bid, which includes solution and corresponding driver
/// data, progressing through the winner selection process.
///
/// It uses the type-state pattern to enforce correct state
/// transitions at compile time. The state parameter tracks progression through
/// three phases:
///
/// 1. **Unscored**: Initial state when the solution is received from the driver
/// 2. **Scored**: After computing surplus and fees for the solution
/// 3. **Ranked**: After winner selection determines if this is a winner
#[derive(Clone)]
pub struct Bid<State = Ranked> {
    solution: crate::infra::api::routes::solve::dto::solve_response::Solution,
    state: State,
}

impl<T> Bid<T> {
    pub fn solution(&self) -> &crate::infra::api::routes::solve::dto::solve_response::Solution {
        &self.solution
    }

    pub fn submission_address(&self) -> &Address {
        &self.solution.submission_address
    }
}

impl<State> state::HasState for Bid<State> {
    type Next<NewState> = Bid<NewState>;
    type State = State;

    fn with_state<NewState>(self, state: NewState) -> Self::Next<NewState> {
        Bid {
            solution: self.solution,
            state,
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl Bid<Unscored> {
    pub fn new(solution: crate::infra::api::routes::solve::dto::solve_response::Solution) -> Self {
        Self {
            solution,
            state: Unscored,
        }
    }
}

pub struct Ranking {
    /// Solutions that were discarded because they were malformed
    /// in some way or deemed unfair by the selection mechanism.
    filtered_out: Vec<Bid<Ranked>>,
    /// Final ranking of the solutions that passed the fairness
    /// check. Winners come before non-winners and higher total
    /// scores come before lower scores.
    ranked: Vec<Bid<Ranked>>,
    /// Reference scores for each winning solver, used to compute rewards.
    reference_scores: HashMap<Address, Score>,
}

impl Ranking {
    /// All solutions including the ones that got filtered out.
    pub fn all(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter().chain(&self.filtered_out)
    }

    /// Enumerates all solutions. The index is used as solution UID.
    pub fn enumerated(&self) -> impl Iterator<Item = (usize, &Bid<Ranked>)> {
        self.all().enumerate()
    }

    /// All solutions that won the right to get executed.
    pub fn winners(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter().filter(|b| b.is_winner())
    }

    /// All solutions that were not filtered out but also did not win.
    pub fn non_winners(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter().filter(|b| !b.is_winner())
    }

    /// Reference scores for each winning solver, used to compute rewards.
    pub fn reference_scores(&self) -> &HashMap<Address, Score> {
        &self.reference_scores
    }

    /// All solutions that passed the filtering step.
    pub fn ranked(&self) -> impl Iterator<Item = &Bid<Ranked>> {
        self.ranked.iter()
    }
}

fn u64_to_be_bytes(x: u64) -> [u8; 8] {
    x.to_be_bytes()
}

fn u256_to_be_bytes(x: &crate::domain::eth::U256) -> [u8; 32] {
    x.to_be_bytes()
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

    buf.extend_from_slice(order.sell_token.0.as_slice());
    buf.extend_from_slice(&u256_to_be_bytes(&order.limit_sell));

    buf.extend_from_slice(order.buy_token.0.as_slice());
    buf.extend_from_slice(&u256_to_be_bytes(&order.limit_buy));

    buf.extend_from_slice(&u256_to_be_bytes(&order.executed_sell));
    buf.extend_from_slice(&u256_to_be_bytes(&order.executed_buy));
}

pub fn hash_solution(
    solution: &crate::infra::api::routes::solve::dto::solve_response::Solution,
) -> [u8; 32] {
    let mut data = Vec::new();

    data.extend_from_slice(&u64_to_be_bytes(solution.solution_id));
    data.extend_from_slice(solution.submission_address.0.as_slice());

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

    let mut prices: Vec<(&eth::Address, &crate::domain::eth::U256)> =
        solution.clearing_prices.iter().collect();
    prices.sort_by(|(a, _), (b, _)| a.cmp(b));

    data.extend_from_slice(&u64_to_be_bytes(prices.len() as u64));

    for (token, price) in prices {
        data.extend_from_slice(token.0.as_slice());
        data.extend_from_slice(&u256_to_be_bytes(price));
    }

    keccak256(&data).into()
}
