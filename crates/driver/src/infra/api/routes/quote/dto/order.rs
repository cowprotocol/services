use {
    crate::{
        domain::{competition, eth},
        infra::time,
        util::serialize,
    },
    serde::Deserialize,
    serde_with::serde_as,
};

impl Order {
    pub fn into_domain(self, now: time::Now) -> Result<competition::quote::Order, Error> {
        Ok(competition::quote::Order {
            sell_token: self.sell_token.into(),
            buy_token: self.buy_token.into(),
            amount: self.amount.into(),
            side: match self.kind {
                Kind::Sell => competition::order::Side::Sell,
                Kind::Buy => competition::order::Side::Buy,
            },
            gas_price: self.effective_gas_price.into(),
            deadline: competition::quote::Deadline::new(self.deadline, now)
                .map_err(|competition::quote::DeadlineExceeded| Error::DeadlineExceeded)?,
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    sell_token: eth::H160,
    buy_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
    kind: Kind,
    #[serde_as(as = "serialize::U256")]
    effective_gas_price: eth::U256,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("received an order with an exceeded deadline")]
    DeadlineExceeded,
}
