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
            quote_id: self.quote_id.map(quote::Id),
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
    /// Id of the quote, allocated by the orderbook. Passed on to the solver as
    /// the id of the quote auction, and the key a fast-path solution is cached
    /// under for a later `/settle`. Absent for native-price estimation, which
    /// prices a token rather than computing a quote anyone can trade on.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deserialize a query string the way the `/quote` route does.
    fn from_query(query: &str) -> Order {
        let uri: axum::http::Uri = format!("http://x/?{query}").parse().unwrap();
        axum::extract::Query::<Order>::try_from_uri(&uri).unwrap().0
    }

    /// Native-price estimation prices a token rather than computing a quote
    /// anyone can trade on, so it sends no `quoteId`. Requiring the field made
    /// the driver reject those requests outright, which surfaced as
    /// `failed to estimate native token price` in the autopilot.
    #[test]
    fn quote_request_without_a_quote_id_is_accepted() {
        let query = "sellToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&\
                     buyToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
                     amount=1000000000000000000&kind=buy&deadline=2026-09-18T09%3A32%3A25.\
                     941283829Z";
        let order: Order = from_query(query);
        assert_eq!(order.quote_id, None);
        assert_eq!(order.into_domain().quote_id, None);
    }

    #[test]
    fn quote_request_with_a_quote_id_keeps_it() {
        let query = "sellToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&\
                     buyToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
                     amount=1000000000000000000&kind=buy&deadline=2026-09-18T09%3A32%3A25.\
                     941283829Z&quoteId=42";
        let order: Order = from_query(query);
        assert_eq!(order.into_domain().quote_id, Some(quote::Id(42)));
    }
}
