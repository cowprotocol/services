use {
    super::OrderUid,
    crate::{boundary::Amounts, domain::eth},
    alloy::primitives::Address,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Quote {
    pub order_uid: OrderUid,
    pub sell_amount: eth::SellTokenAmount,
    pub buy_amount: eth::TokenAmount,
    pub fee: eth::SellTokenAmount,
    pub solver: Address,
}

impl From<&Quote> for Amounts {
    fn from(quote: &Quote) -> Self {
        Self {
            sell: quote.sell_amount.0,
            buy: quote.buy_amount.0,
            fee: quote.fee.0,
        }
    }
}

#[cfg(test)]
impl Default for Quote {
    fn default() -> Self {
        Self {
            order_uid: OrderUid([0; 56]),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            fee: Default::default(),
            solver: Default::default(),
        }
    }
}
