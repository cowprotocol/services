use {
    super::auction::order,
    crate::domain::{self, auction, eth},
    alloy::primitives::Address,
    derive_more::Display,
    num::Saturating,
    std::collections::HashMap,
};

mod bid;
pub mod winner_selection;

use ::winner_selection::solution_hash::{HashableSolution, HashableTradedOrder};
pub use bid::{Bid, RankType, Ranked, Scored, Unscored};

type SolutionId = u64;

#[derive(Debug, Clone)]
pub struct Solution {
    /// A solution ID provided by the solver.
    id: SolutionId,
    solver: Address,
    orders: HashMap<domain::OrderUid, TradedOrder>,
    prices: auction::Prices,
}

impl Solution {
    pub fn new(
        id: SolutionId,
        solver: Address,
        orders: HashMap<domain::OrderUid, TradedOrder>,
        prices: auction::Prices,
    ) -> Self {
        Self {
            id,
            solver,
            orders,
            prices,
        }
    }
}

impl Solution {
    pub fn id(&self) -> SolutionId {
        self.id
    }

    pub fn solver(&self) -> Address {
        self.solver
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

impl HashableSolution for Solution {
    type Order = TradedOrder;
    type OrderId = domain::OrderUid;
    type TokenAddr = eth::TokenAddress;

    fn solution_id(&self) -> u64 {
        self.id
    }

    fn solver_address(&self) -> &[u8] {
        self.solver.as_slice()
    }

    fn orders(&self) -> impl Iterator<Item = (&Self::OrderId, &Self::Order)> {
        self.orders.iter()
    }

    fn prices(&self) -> impl Iterator<Item = (&Self::TokenAddr, [u8; 32])> {
        self.prices
            .iter()
            .map(|(token, price)| (token, price.get().0.to_be_bytes()))
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

impl HashableTradedOrder for TradedOrder {
    fn side_byte(&self) -> u8 {
        match self.side {
            order::Side::Buy => 0,
            order::Side::Sell => 1,
        }
    }

    fn sell_token(&self) -> &[u8] {
        self.sell.token.0.as_slice()
    }

    fn sell_amount(&self) -> [u8; 32] {
        self.sell.amount.0.to_be_bytes()
    }

    fn buy_token(&self) -> &[u8] {
        self.buy.token.0.as_slice()
    }

    fn buy_amount(&self) -> [u8; 32] {
        self.buy.amount.0.to_be_bytes()
    }

    fn executed_sell(&self) -> [u8; 32] {
        self.executed_sell.0.to_be_bytes()
    }

    fn executed_buy(&self) -> [u8; 32] {
        self.executed_buy.0.to_be_bytes()
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
pub struct Score(pub eth::Ether);

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
pub enum SolutionError {
    #[error(transparent)]
    ZeroScore(#[from] ZeroScore),
    #[error(transparent)]
    InvalidPrice(#[from] auction::InvalidPrice),
    #[error("the solver got deny listed")]
    SolverDenyListed,
}
