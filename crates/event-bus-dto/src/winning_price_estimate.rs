use {
    crate::{Event, query::QueryFields},
    schemars::JsonSchema,
    serde::Serialize,
};

/// Emitted once price-estimate competition has picked the winning estimate that
/// becomes the official quote. Its job is correlation: among the
/// [`crate::PriceEstimateEvent`]s emitted for the same request (one per
/// competing estimator, all sharing the envelope `requestId`), the winning one
/// is the one whose `estimator` matches this event.
///
/// At most one is emitted per quote competition (because all may have errored
/// in which case there's effectively no winner).
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WinningPriceEstimateEvent {
    pub query: QueryFields,
    /// Name of the estimator whose price estimate won the competition.
    pub estimator: String,
}

impl Event for WinningPriceEstimateEvent {
    const SUBJECT: &'static str = "winningPriceEstimate";
}

#[cfg(test)]
mod tests {
    use {super::*, crate::query::OrderKind, serde_json::json};

    #[test]
    fn matches_wire_format() {
        let event = WinningPriceEstimateEvent {
            query: QueryFields {
                sell_token: "0x01".into(),
                buy_token: "0x02".into(),
                in_amount: "100".into(),
                kind: OrderKind::Sell,
            },
            estimator: "baseline".into(),
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
                "estimator": "baseline",
            }),
        );
    }
}
