use {schemars::JsonSchema, serde::Serialize};

// The token query shared by quote-related events. `QuoteRequestedEvent` and
// `PriceEstimateEvent` emitted for the same request carry an identical query,
// and consumers correlate them by the envelope's `requestId` expecting the
// exact same shape. Keeping a single definition here guarantees the two can't
// drift apart.
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
