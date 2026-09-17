use {
    crate::domain::{competition, quote},
    eth_domain_types as eth,
    serde::Deserialize,
    serde_with::serde_as,
};

impl Order {
    pub fn into_domain(self) -> Result<quote::Order, Error> {
        Ok(quote::Order {
            tokens: quote::Tokens::new(self.sell_token.into(), self.buy_token.into()),
            amount: self.amount.into(),
            side: match self.kind {
                Kind::Sell => competition::order::Side::Sell,
                Kind::Buy => competition::order::Side::Buy,
            },
            deadline: self.deadline,
            enable_fast_path: self.enable_fast_path,
            quote_id: self.quote_id.map(quote::Id).ok_or(Error::MissingQuoteId)?,
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    sell_token: eth::Address,
    buy_token: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    amount: eth::U256,
    kind: Kind,
    deadline: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    enable_fast_path: bool,
    /// Id of the quote, allocated by the orderbook and sent with every quote
    /// request. Passed on to the solver as the id of the quote auction, and
    /// the key a fast-path solution is cached under for a later `/settle`.
    /// Deserialized as optional only to report its absence as a proper error.
    #[serde(default)]
    quote_id: Option<i64>,
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
    #[error("received a quote request without a quote id")]
    MissingQuoteId,
}
