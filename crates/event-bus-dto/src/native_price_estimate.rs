use {crate::Event, alloy_primitives::Address, schemars::JsonSchema, serde::Serialize};

/// Emitted once per estimator taking part in a native price competition, as
/// soon as that estimator returns. Because the native price cache absorbs the
/// vast majority of lookups, these events describe the price *refreshes* that
/// actually reached an estimator, not every native price the protocol used.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NativePriceEstimateEvent {
    /// Token the price was estimated for. For tokens configured to be
    /// approximated by another token this is the approximation token, i.e. the
    /// one actually priced.
    #[schemars(with = "String")]
    pub token: Address,
    /// Wall-clock time the estimator actually spent, in milliseconds.
    pub elapsed: u64,
    pub estimator: String,
    /// Amount of native token needed to buy 1 unit of the token, or the error
    /// the estimator failed with. Prices are always normal, positive floats:
    /// malformed prices are reported as errors.
    pub result: Result<f64, String>,
}

impl Event for NativePriceEstimateEvent {
    const SUBJECT: &'static str = "nativePriceEstimate";
}

#[cfg(test)]
mod tests {
    use {super::*, alloy_primitives::address, serde_json::json};

    #[test]
    fn matches_wire_format() {
        let event = NativePriceEstimateEvent {
            token: address!("0x0000000000000000000000000000000000000001"),
            elapsed: 12,
            estimator: "CoinGecko".into(),
            result: Ok(1.5e-13),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({
                "token": "0x0000000000000000000000000000000000000001",
                "elapsed": 12,
                "estimator": "CoinGecko",
                "result": { "Ok": 1.5e-13 },
            }),
        );
    }

    #[test]
    fn error_wire_format() {
        let event = NativePriceEstimateEvent {
            token: address!("0x0000000000000000000000000000000000000001"),
            elapsed: 12,
            estimator: "CoinGecko".into(),
            result: Err("boom".into()),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap()["result"],
            json!({ "Err": "boom" }),
        );
    }
}
