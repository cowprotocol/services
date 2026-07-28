use {
    crate::{Event, query::QueryFields},
    schemars::JsonSchema,
    serde::Serialize,
};

/// Emitted for a validated quote request, just before price estimation runs.
/// Carries the request context that partner and quote analysis care about —
/// notably the `appCode` and the token symbols — which are not present on the
/// per-estimator [`crate::PriceEstimateEvent`].
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequestedEvent {
    pub query: QueryFields,
    /// Caller address (hex-encoded, including the `0x` prefix).
    pub from: String,
    /// `appCode` from the order's app-data document, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// Symbol of the sell token, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sell_token_symbol: Option<String>,
    /// Symbol of the buy token, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buy_token_symbol: Option<String>,
    pub price_quality: PriceQuality,
}

impl Event for QuoteRequestedEvent {
    const SUBJECT: &'static str = "quoteRequested";
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PriceQuality {
    Fast,
    Optimal,
    Verified,
}

#[cfg(test)]
mod tests {
    use {super::*, crate::query::OrderKind, serde_json::json};

    #[test]
    fn matches_wire_format() {
        let event = QuoteRequestedEvent {
            query: QueryFields {
                sell_token: "0x01".into(),
                buy_token: "0x02".into(),
                in_amount: "100".into(),
                kind: OrderKind::Sell,
            },
            from: "0x0000000000000000000000000000000000000000".into(),
            app_code: Some("CoW Swap".into()),
            sell_token_symbol: Some("WETH".into()),
            buy_token_symbol: Some("USDC".into()),
            price_quality: PriceQuality::Optimal,
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({
                "query": {
                    "sellToken": "0x01",
                    "buyToken": "0x02",
                    "inAmount": "100",
                    "kind": "sell",
                },
                "from": "0x0000000000000000000000000000000000000000",
                "appCode": "CoW Swap",
                "sellTokenSymbol": "WETH",
                "buyTokenSymbol": "USDC",
                "priceQuality": "optimal",
            }),
        );
    }

    #[test]
    fn omits_missing_optionals() {
        let event = QuoteRequestedEvent {
            query: QueryFields {
                sell_token: "0x01".into(),
                buy_token: "0x02".into(),
                in_amount: "100".into(),
                kind: OrderKind::Buy,
            },
            from: "0x0000000000000000000000000000000000000000".into(),
            app_code: None,
            sell_token_symbol: None,
            buy_token_symbol: None,
            price_quality: PriceQuality::Fast,
        };
        let value = serde_json::to_value(&event).unwrap();
        assert!(value.get("appCode").is_none());
        assert!(value.get("sellTokenSymbol").is_none());
        assert!(value.get("buyTokenSymbol").is_none());
    }
}
