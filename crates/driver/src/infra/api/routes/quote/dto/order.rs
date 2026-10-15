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
            quote_id: quote::Id(self.quote_id),
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
    quote_id: i64,
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

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: &str = "sellToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&\
                         buyToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
                         amount=1000000000000000000&kind=buy&deadline=2026-09-18T09%3A32%3A25.\
                         941283829Z";

    /// Deserialize a query string the way the `/quote` route does.
    fn from_query(query: &str) -> Result<Order, axum::extract::rejection::QueryRejection> {
        let uri: axum::http::Uri = format!("http://x/?{query}").parse().unwrap();
        axum::extract::Query::<Order>::try_from_uri(&uri).map(|query| query.0)
    }

    /// Every quote request carries the id the orderbook allocated for it,
    /// native-price estimation included, so a request without one is
    /// malformed.
    #[test]
    fn quote_request_without_a_quote_id_is_rejected() {
        assert!(from_query(ORDER).is_err());
    }

    #[test]
    fn quote_request_with_a_quote_id_keeps_it() {
        let order = from_query(&format!("{ORDER}&quoteId=42")).unwrap();
        assert_eq!(order.into_domain().quote_id, quote::Id(42));
    }
}
