use {crate::Event, schemars::JsonSchema, serde::Serialize};

/// Emitted once per estimator taking part in a native price competition, as
/// soon as that estimator returns. Because the native price cache absorbs the
/// vast majority of lookups, these events describe the price *refreshes* that
/// actually reached an estimator, not every native price the protocol used.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NativePriceEstimateEvent {
    /// Token the price was estimated for (hex-encoded, including the `0x`
    /// prefix). For tokens configured to be approximated by another token this
    /// is the approximation token, i.e. the one actually priced.
    pub token: String,
    /// Timeout granted to the estimator's competition stage, in milliseconds.
    pub timeout: u64,
    /// Wall-clock time the estimator actually spent, in milliseconds.
    pub elapsed: u64,
    pub estimator: String,
    pub result: NativePriceResult,
}

impl Event for NativePriceEstimateEvent {
    const SUBJECT: &'static str = "nativePriceEstimate";
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum NativePriceResult {
    Ok {
        /// Amount of native token needed to buy 1 unit of the token. Always a
        /// normal, positive float: malformed prices are reported as errors.
        price: f64,
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
        let event = NativePriceEstimateEvent {
            token: "0x01".into(),
            timeout: 5000,
            elapsed: 12,
            estimator: "CoinGecko".into(),
            result: NativePriceResult::Ok { price: 1.5e-13 },
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({
                "token": "0x01",
                "timeout": 5000,
                "elapsed": 12,
                "estimator": "CoinGecko",
                "result": {
                    "price": 1.5e-13,
                },
            }),
        );
    }

    #[test]
    fn error_variant_is_untagged() {
        let result = NativePriceResult::Err {
            error: "boom".into(),
        };
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({ "error": "boom" }),
        );
    }
}
