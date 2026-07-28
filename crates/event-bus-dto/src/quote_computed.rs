use {crate::Event, schemars::JsonSchema, serde::Serialize};

/// Emitted once the orderbook has finished computing a quote ("finished
/// computing quote"). Its main job is correlation: the envelope's `requestId`
/// ties it back to the [`crate::QuoteRequestedEvent`] and the per-estimator
/// [`crate::PriceEstimateEvent`]s of the same request, while `quoteId` links
/// forward to the resulting order via the `quotes` table.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuoteComputedEvent {
    /// Database id of the stored quote. Absent for `fast` quotes, which are
    /// not persisted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<i64>,
    /// `appCode` from the order's app-data document, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// Whether the quote was verified by simulation.
    pub verified: bool,
}

impl Event for QuoteComputedEvent {
    const SUBJECT: &'static str = "quoteComputed";
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn matches_wire_format() {
        let event = QuoteComputedEvent {
            quote_id: Some(42),
            app_code: Some("CoW Swap".into()),
            verified: true,
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({
                "quoteId": 42,
                "appCode": "CoW Swap",
                "verified": true,
            }),
        );
    }

    #[test]
    fn omits_missing_optionals() {
        let event = QuoteComputedEvent {
            quote_id: None,
            app_code: None,
            verified: false,
        };
        let value = serde_json::to_value(&event).unwrap();
        assert!(value.get("quoteId").is_none());
        assert!(value.get("appCode").is_none());
        assert_eq!(value.get("verified"), Some(&json!(false)));
    }
}
