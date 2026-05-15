pub use winner_selection::Unscored;
use {
    crate::{domain::competition::order::FeePolicy, infra::api::routes::solve::dto},
    ::observe::metrics,
    eth_domain_types::{self as eth, Ether, WrappedNativeToken},
    winner_selection::{
        self as winsel,
        OrderUid,
        state::{HasState, RankedItem, ScoredItem, UnscoredItem},
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
        self.arbitrate_with_context(bids, &auction.into())
    }

    /// Same as [`Self::arbitrate`] but takes a precomputed
    /// [`winsel::AuctionContext`]. Use this when the caller has already
    /// built the context in the foreground, to avoid the per-call
    /// conversion from `Auction`.
    pub fn arbitrate_with_context(
        &self,
        bids: Vec<Bid<Unscored>>,
        context: &winsel::AuctionContext,
    ) -> Vec<Bid> {
        let paired = bids
            .into_iter()
            .map(|bid| {
                let solution: winsel::Solution<winsel::Unscored> = bid.payload().into();
                (bid, solution)
            })
            .collect();
        let rejoined = self.0.arbitrate_paired_and_rejoin(paired, context);

        // An orphan means two input bids shared a `SolutionKey`. Pod is
        // open so untrusted input can engineer such collisions. Don't panic
        // (it would kill the spawned pod task); warn and bump the counter
        // so oncall can alert.
        if rejoined.orphans > 0 {
            tracing::warn!(
                orphans = rejoined.orphans,
                "ranked solutions had no matching bid; SolutionKey collision suspected",
            );
            Metrics::get()
                .orphan_solutions
                .inc_by(rejoined.orphans as u64);
        }
        debug_assert!(rejoined.orphans == 0, "expected no orphans");

        rejoined
            .ranked
            .into_iter()
            .map(|(bid, ws_solution)| {
                bid.with_score(Score(eth::Ether(ws_solution.score())))
                    .with_rank(ws_solution.state().rank_type)
            })
            .collect()
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "winner_selection")]
struct Metrics {
    /// Arbitrator-returned solutions whose `SolutionKey` had no matching
    /// bid in the rejoin step. Non-zero indicates a `SolutionKey` collision
    /// in the input set or an arbitrator invariant violation.
    orphan_solutions: prometheus::IntCounter,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(metrics::get_storage_registry()).unwrap()
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

/// A solver's auction bid in the typestate pipeline `Unscored -> Scored ->
/// Ranked`. State transitions are enforced at compile time via
/// [`winsel::Bid`].
pub type Bid<State = Ranked> = winsel::Bid<dto::solve_response::Solution, State>;
