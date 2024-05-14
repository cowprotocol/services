use {
    super::{auction, eth},
    crate::domain,
    number::nonzero::U256 as NonZeroU256,
    std::collections::HashMap,
};

type SolutionId = u64;

pub struct Solution {
    id: SolutionId,
    account: eth::Address,
    score: NonZeroU256,
    orders: HashMap<domain::OrderUid, TradedAmounts>,
    // uniform prices for all tokens
    prices: HashMap<eth::TokenAddress, auction::Price>,
}

impl Solution {
    pub fn new(
        id: SolutionId,
        account: eth::Address,
        score: NonZeroU256,
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

    pub fn score(&self) -> NonZeroU256 {
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
