use {
    crate::domain::{competition::order::FeePolicy, eth::Address},
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
    ) -> Vec<Bid> {
        let mut bids = bids;
        bids.sort_by_cached_key(|b| winsel::solution_hash::hash_solution(b.solution()));

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

        ranked
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
