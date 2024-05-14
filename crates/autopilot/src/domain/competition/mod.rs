use {
    super::{auction, eth},
    crate::domain,
    std::collections::HashMap,
};

type SolutionId = u64;

pub struct Solution {
    id: SolutionId,
    account: eth::Address,
    score: Score,
    orders: HashMap<domain::OrderUid, TradedAmounts>,
    // uniform prices for all tokens
    prices: HashMap<eth::TokenAddress, auction::Price>,
}

impl Solution {
    pub fn new(
        id: SolutionId,
        account: eth::Address,
        score: Score,
        orders: HashMap<domain::OrderUid, TradedAmounts>,
        prices: HashMap<eth::TokenAddress, auction::Price>,
    ) -> Self {
        Self {
            id,
            account,
            score,
            orders,
            prices,
        }
    }

    pub fn id(&self) -> SolutionId {
        self.id
    }

    pub fn account(&self) -> eth::Address {
        self.account
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

pub struct TradedAmounts {
    /// The effective amount that left the user's wallet including all fees.
    pub sell: eth::TokenAmount,
    /// The effective amount the user received after all fees.
    pub buy: eth::TokenAmount,
}

#[derive(Debug, Clone, Copy)]
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
