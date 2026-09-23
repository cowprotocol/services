use {
    crate::{Event, query::QueryFields},
    alloy_primitives::Address,
    schemars::JsonSchema,
    serde::Serialize,
};

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PriceEstimateEvent {
    pub query: QueryFields,
    /// Caller address (hex-encoded, including the `0x` prefix).
    pub from: String,
    /// Total timeout granted to the estimator, in milliseconds.
    pub timeout: u64,
    /// Wall-clock time the estimator actually spent, in milliseconds.
    pub elapsed: u64,
    pub estimator: String,
    /// Settlement address of the solver behind `estimator`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub solver: Option<Address>,
    /// Absent when the estimator failed, and for estimates no solver produced
    /// (trivial ETH/WETH quotes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<i64>,
    pub result: EstimateResult,
}

impl Event for PriceEstimateEvent {
    const SUBJECT: &'static str = "priceEstimate";
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum EstimateResult {
    #[serde(rename_all = "camelCase")]
    Ok {
        /// Decimal-encoded output amount.
        out_amount: String,
        /// Decimal-encoded gas estimate.
        gas: String,
        verified: bool,
    },
    Err {
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use {super::*, crate::query::OrderKind, alloy_primitives::address, serde_json::json};

    #[test]
    fn matches_wire_format() {
        let event = PriceEstimateEvent {
            query: QueryFields {
                sell_token: "0x01".into(),
                buy_token: "0x02".into(),
                in_amount: "100".into(),
                kind: OrderKind::Sell,
            },
            from: "0x0000000000000000000000000000000000000000".into(),
            timeout: 5000,
            elapsed: 12,
            estimator: "baseline".into(),
            solver: Some(address!("0x0000000000000000000000000000000000000003")),
            quote_id: Some(42),
            result: EstimateResult::Ok {
                out_amount: "99".into(),
                gas: "21000".into(),
                verified: true,
            },
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
                "timeout": 5000,
                "elapsed": 12,
                "estimator": "baseline",
                "solver": "0x0000000000000000000000000000000000000003",
                "quoteId": 42,
                "result": {
                    "outAmount": "99",
                    "gas": "21000",
                    "verified": true,
                },
            }),
        );
    }

    #[test]
    fn error_variant_is_untagged() {
        let result = EstimateResult::Err {
            error: "boom".into(),
        };
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({ "error": "boom" }),
        );
    }
}
