use {schemars::JsonSchema, serde::Serialize};

// The token query shared by quote-related events, kept in one place so the
// `QuoteRequestedEvent` and `PriceEstimateEvent` for the same trade can't drift
// apart.
//
// Note that a single `requestId` carries several `priceEstimate` events: the
// quote competition emits one per competing estimator, all sharing this same
// query. Consumers disambiguate them by `estimator`, not `requestId` alone.
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
