use {crate::Event, alloy_primitives::Address, schemars::JsonSchema, serde::Serialize};

/// Emitted once a native price competition has picked the winning estimate,
/// i.e. the price that gets cached and used to price orders. Its job is
/// correlation: among the [`crate::NativePriceEstimateEvent`]s emitted for the
/// same token around the same time, the winning one is the one whose
/// `estimator` matches this event.
///
/// At most one is emitted per competition, as all estimators may have errored
/// in which case there is no winner.
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
