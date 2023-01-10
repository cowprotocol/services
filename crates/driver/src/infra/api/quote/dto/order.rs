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
            amount: match self.amount {
                Amount::Sell { sell_amount } => competition::quote::Amount::Sell(sell_amount),
                Amount::Buy { buy_amount } => competition::quote::Amount::Buy(buy_amount),
            },
            valid_to: self.valid_to,
            partial: self.partially_fillable,
            quality: match self.price_quality {
                PriceQuality::Optimal => competition::quote::Quality::Optimal,
                PriceQuality::Fast => competition::quote::Quality::Fast,
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
    amount: Amount,
    valid_to: u32,
    #[serde(default)]
    partially_fillable: bool,
    #[serde(default)]
    price_quality: PriceQuality,
    #[serde_as(as = "serialize::U256")]
    effective_gas_price: eth::U256,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
enum Amount {
    Sell { sell_amount: eth::U256 },
    Buy { buy_amount: eth::U256 },
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceQuality {
    #[default]
    Optimal,
    Fast,
}
