use {crate::Event, alloy_primitives::Address, schemars::JsonSchema, serde::Serialize};

/// Emitted once a native price competition picks the estimate that gets cached,
/// and not at all when every estimator errored. Correlate on `estimator` with
/// the [`crate::NativePriceEstimateEvent`]s for the same token.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WinningNativePriceEstimateEvent {
    /// Token the price was estimated for.
    #[schemars(with = "String")]
    pub token: Address,
    /// Name of the estimator whose price estimate won the competition.
    pub estimator: String,
}

impl Event for WinningNativePriceEstimateEvent {
    const SUBJECT: &'static str = "winningNativePriceEstimate";
}

#[cfg(test)]
mod tests {
    use {super::*, alloy_primitives::address, serde_json::json};

    #[test]
    fn matches_wire_format() {
        let event = WinningNativePriceEstimateEvent {
            token: address!("0x0000000000000000000000000000000000000001"),
            estimator: "CoinGecko".into(),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({
                "token": "0x0000000000000000000000000000000000000001",
                "estimator": "CoinGecko",
            }),
        );
    }
}
