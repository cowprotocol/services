use {
    crate::domain::{competition, quote},
    eth_domain_types as eth,
    serde::Deserialize,
    serde_with::serde_as,
};

impl Order {
    pub fn into_domain(self) -> quote::Order {
        quote::Order {
            tokens: quote::Tokens::new(self.sell_token.into(), self.buy_token.into()),
            amount: self.amount.into(),
            side: match self.kind {
                Kind::Sell => competition::order::Side::Sell,
                Kind::Buy => competition::order::Side::Buy,
            },
            deadline: self.deadline,
            enable_fast_path: self.enable_fast_path,
            auction_id: self.auction_id,
        }
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
    /// Real auction id the orderbook allocated from the shared `auctions`
    /// sequence, under which a fast-path solution is cached for a later
    /// `/settle`. Only present for fast-path quotes.
    #[serde(default)]
    auction_id: Option<i64>,
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
