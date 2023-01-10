use {
    crate::domain::{competition, eth},
    serde::Serialize,
    serde_with::serde_as,
};

impl Quote {
    pub fn from_domain(quote: &competition::quote::Quote) -> Self {
        Self {
            sell_token: quote.sell.token.into(),
            buy_token: quote.buy.token.into(),
            sell_amount: quote.sell.amount,
            buy_amount: quote.buy.amount,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub sell_token: eth::H160,
    pub buy_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    pub sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    pub buy_amount: eth::U256,
}
