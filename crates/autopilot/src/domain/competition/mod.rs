use {
    crate::{
        domain,
        domain::{auction, eth},
    },
    derive_more::Display,
    std::collections::HashMap,
};

type SolutionId = u64;

#[derive(Debug)]
pub struct SolutionWithId {
    id: SolutionId,
    solution: Solution,
}

impl SolutionWithId {
    pub fn new(
        id: SolutionId,
        solver: eth::Address,
        score: Score,
        orders: HashMap<domain::OrderUid, TradedAmounts>,
        prices: auction::Prices,
    ) -> Self {
        Self {
            id,
            solution: Solution::new(solver, score, orders, prices),
        }
    }

    pub fn id(&self) -> SolutionId {
        self.id
    }

    pub fn solver(&self) -> eth::Address {
        self.solution.solver()
    }

    pub fn score(&self) -> Score {
        self.solution.score()
    }

    pub fn order_ids(&self) -> impl Iterator<Item = &domain::OrderUid> {
        self.solution.order_ids()
    }

    pub fn orders(&self) -> &HashMap<domain::OrderUid, TradedAmounts> {
        self.solution.orders()
    }

    pub fn prices(&self) -> &HashMap<eth::TokenAddress, auction::Price> {
        self.solution.prices()
    }
}

#[derive(Debug)]
pub struct Solution {
    solver: eth::Address,
    score: Score,
    orders: HashMap<domain::OrderUid, TradedAmounts>,
    prices: auction::Prices,
}

impl Solution {
    pub fn new(
        solver: eth::Address,
        score: Score,
        orders: HashMap<domain::OrderUid, TradedAmounts>,
        prices: auction::Prices,
    ) -> Self {
        Self {
            solver,
            score,
            orders,
            prices,
        }
    }

    pub fn solver(&self) -> eth::Address {
        self.solver
    }

    pub fn score(&self) -> Score {
        self.score
    }

    pub fn order_ids(&self) -> impl Iterator<Item = &domain::OrderUid> {
        self.orders.keys()
    }

    pub fn orders(&self) -> &HashMap<domain::OrderUid, TradedAmounts> {
        &self.orders
    }

    pub fn prices(&self) -> &HashMap<eth::TokenAddress, auction::Price> {
        &self.prices
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TradedAmounts {
    /// The effective amount that left the user's wallet including all fees.
    pub sell: eth::TokenAmount,
    /// The effective amount the user received after all fees.
    pub buy: eth::TokenAmount,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Display)]
pub struct Score(eth::Ether);

impl Score {
    pub fn new(score: eth::Ether) -> Result<Self, ZeroScore> {
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
