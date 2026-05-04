pub use winner_selection::Unscored;
use {
    crate::domain::competition::order::FeePolicy,
    eth_domain_types::{self as eth, Address, Ether, WrappedNativeToken},
    winner_selection::{
        self as winsel,
        OrderUid,
        state::{self, HasState, RankedItem, ScoredItem, UnscoredItem},
    },
};

/// Score for a solution, wrapping the surplus value.
#[derive(Debug, Clone, Copy)]
pub struct Score(pub Ether);

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
        let token: eth::TokenAddress = *wrapped_native_token;
        Self(winsel::Arbitrator {
            max_winners,
            weth: *token,
        })
    }

    /// Runs the entire auction mechanism on the passed in solutions.
    pub fn arbitrate(
        &self,
        bids: Vec<Bid<Unscored>>,
        auction: &crate::domain::competition::Auction,
    ) -> Vec<Bid> {
        let paired = bids
            .into_iter()
            .map(|bid| {
                let solution: winsel::Solution<winsel::Unscored> = bid.solution().into();
                (bid, solution)
            })
            .collect();
        let (ws_ranking, mut by_key) = self.0.arbitrate_paired(paired, &auction.into());

        ws_ranking
            .ranked
            .into_iter()
            .map(|ws_solution| {
                let bid = by_key
                    .remove(&winsel::SolutionKey::from(&ws_solution))
                    .expect("every ranked solution has a matching bid");
                bid.with_score(Score(eth::Ether(ws_solution.score())))
                    .with_rank(ws_solution.state().rank_type)
            })
            .collect()
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
                .map(|(token, price)| ((*token).into(), price.price.unwrap().0.0))
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
