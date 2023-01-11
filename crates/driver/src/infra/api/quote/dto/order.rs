use {
    crate::domain::{competition, eth},
    serde::Deserialize,
    serde_with::serde_as,
};

impl Order {
    pub fn into_domain(self) -> competition::quote::Order {
        competition::quote::Order {
            sell_token: self.sell_token.into(),
            buy_token: self.buy_token.into(),
            amount: self.amount.into(),
            side: match self.side {
                Side::Sell => competition::order::Side::Sell,
                Side::Buy => competition::order::Side::Buy,
            },
            gas_price: self.effective_gas_price.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde_as]
pub struct Order {
    sell_token: eth::H160,
    buy_token: eth::H160,
    amount: eth::U256,
    side: Side,
    #[serde_as(as = "serialize::U256")]
    effective_gas_price: eth::U256,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Sell,
    Buy,
}
