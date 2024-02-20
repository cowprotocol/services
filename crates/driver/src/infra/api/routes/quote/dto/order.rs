use {
    crate::{
        domain::{competition, eth, quote, time},
        infra::solver::Timeouts,
    },
    number::U256,
    serde::Deserialize,
};

impl Order {
    pub fn into_domain(self, timeouts: Timeouts) -> Result<quote::Order, Error> {
        Ok(quote::Order {
            tokens: quote::Tokens::new(self.sell_token.into(), self.buy_token.into())
                .map_err(|quote::SameTokens| Error::SameTokens)?,
            amount: eth::U256::from(self.amount).into(),
            side: match self.kind {
                Kind::Sell => competition::order::Side::Sell,
                Kind::Buy => competition::order::Side::Buy,
            },
            deadline: time::Deadline::new(self.deadline, timeouts),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Order {
    sell_token: eth::H160,
    buy_token: eth::H160,
    amount: U256,
    kind: Kind,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("received an order with identical buy and sell tokens")]
    SameTokens,
}
