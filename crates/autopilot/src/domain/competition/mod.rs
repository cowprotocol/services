use {
    super::auction::order,
    crate::domain::{self, auction, eth},
    derive_more::Display,
    std::collections::HashMap,
};

mod participant;
mod participation_guard;
pub mod winner_selection;

pub use {
    participant::{Participant, Ranked, Unranked},
    participation_guard::SolverParticipationGuard,
};

type SolutionId = u64;

#[derive(Debug, Clone)]
pub struct Solution {
    id: SolutionId,
    solver: eth::Address,
    /// Score reported by the solver in their response.
    score: Score,
    orders: HashMap<domain::OrderUid, TradedOrder>,
    prices: auction::Prices,
    /// Score computed by the autopilot based on the solution
    /// of the solver.
    // TODO: refactor this to compute the score in the constructor
    computed_score: Option<Score>,
}

impl Solution {
    pub fn new(
        id: SolutionId,
        solver: eth::Address,
        score: Score,
        orders: HashMap<domain::OrderUid, TradedOrder>,
        prices: auction::Prices,
    ) -> Self {
        Self {
            id,
            solver,
            score,
            orders,
            prices,
            computed_score: None,
        }
    }

    pub fn id(&self) -> SolutionId {
        self.id
    }

    pub fn solver(&self) -> eth::Address {
        self.solver
    }

    pub fn score(&self) -> Score {
        self.score
    }

    pub fn computed_score(&self) -> Option<&Score> {
        self.computed_score.as_ref()
    }

    pub fn order_ids(&self) -> impl Iterator<Item = &domain::OrderUid> + std::fmt::Debug {
        self.orders.keys()
    }

    pub fn orders(&self) -> &HashMap<domain::OrderUid, TradedOrder> {
        &self.orders
    }

    pub fn prices(&self) -> &HashMap<eth::TokenAddress, auction::Price> {
        &self.prices
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TradedOrder {
    pub side: order::Side,
    /// The sell token and limit sell amount of sell token.
    pub sell: eth::Asset,
    /// The buy token and limit buy amount of buy token.
    pub buy: eth::Asset,
    /// The effective amount that left the user's wallet including all fees.
    pub executed_sell: eth::TokenAmount,
    /// The effective amount the user received after all fees.
    pub executed_buy: eth::TokenAmount,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Display,
    Default,
    derive_more::AddAssign,
    derive_more::Add,
)]
pub struct Score(eth::Ether);

impl Score {
    pub fn try_new(score: eth::Ether) -> Result<Self, ZeroScore> {
        if score.0.is_zero() {
            Err(ZeroScore)
        } else {
            Ok(Self(score))
        }
    }

    pub fn get(&self) -> &eth::Ether {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the solver proposed a 0-score solution")]
pub struct ZeroScore;

#[derive(Debug, thiserror::Error)]
pub enum SolutionError {
    #[error(transparent)]
    ZeroScore(#[from] ZeroScore),
    #[error(transparent)]
    InvalidPrice(#[from] auction::InvalidPrice),
    #[error("the solver got deny listed")]
    SolverDenyListed,
}
