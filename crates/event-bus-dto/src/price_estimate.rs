use {crate::Event, schemars::JsonSchema, serde::Serialize};

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PriceEstimateEvent {
    pub query: QueryFields,
    /// Caller address (hex-encoded, including the `0x` prefix).
    pub from: String,
    /// Total timeout granted to the estimator, in milliseconds.
    pub timeout: u128,
    /// Wall-clock time the estimator actually spent, in milliseconds.
    pub elapsed: u128,
    pub estimator: String,
    pub result: EstimateResult,
}

impl Event for PriceEstimateEvent {
    const SUBJECT: &'static str = "priceEstimate";
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueryFields {
    /// Hex-encoded sell token address.
    pub sell_token: String,
    /// Hex-encoded buy token address.
    pub buy_token: String,
    /// Decimal-encoded input amount (interpretation depends on `kind`).
    pub in_amount: String,
    pub kind: OrderKind,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OrderKind {
    Sell,
    Buy,
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
    use {super::*, serde_json::json};

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
