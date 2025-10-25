use {
    crate::domain::{competition, eth, quote},
    serde::Deserialize,
};

impl Order {
    pub fn into_domain(self) -> Result<quote::Order, Error> {
        Ok(quote::Order {
            tokens: quote::Tokens::try_new(self.sell_token.into(), self.buy_token.into())
                .map_err(|quote::SameTokens| Error::SameTokens)?,
            amount: self.amount.into(),
            side: match self.kind {
                Kind::Sell => competition::order::Side::Sell,
                Kind::Buy => competition::order::Side::Buy,
            },
            deadline: self.deadline,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    sell_token: eth::H160,
    buy_token: eth::H160,
    amount: eth::TokenAmount,
    kind: Kind,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("received an order with identical buy and sell tokens")]
    SameTokens,
}
