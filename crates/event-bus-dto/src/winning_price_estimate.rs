use {
    crate::{Event, query::QueryFields},
    schemars::JsonSchema,
    serde::Serialize,
};

/// Emitted once price-estimate competition has picked the winning estimate that
/// becomes the official quote. Its job is correlation: the winning
/// [`crate::PriceEstimateEvent`] of the same request is the one carrying the
/// matching `query` and `estimator` under the same envelope `requestId`.
///
/// Like [`crate::PriceEstimateEvent`], one is emitted per competition run, so a
/// single `/quote` that also derives native prices yields several — consumers
/// pick the official quote's winner by matching `query` to the traded pair.
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
