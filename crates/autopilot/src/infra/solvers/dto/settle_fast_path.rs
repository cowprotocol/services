use {
    crate::infra::persistence::dto::order::Order,
    alloy::primitives::U256,
    number::serialization::HexOrDecimalU256,
    serde::Serialize,
    serde_with::serde_as,
};

/// Request to the driver's `/settle_fast_path` endpoint: settle the quote
/// solution the driver cached under `quote_id`, re-encoded against the real
/// signed order, outside of a competition.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Id of the quote whose cached solution the driver should settle.
    pub quote_id: i64,
    /// The real signed order the cached solution is re-encoded against.
    pub order: Order,
    /// The sell/buy amounts the order must fill at exactly.
    pub limit_prices: LimitPrices,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID allocated for this settlement. The quote was computed
    /// outside of any auction, but its settlement is still attributed to one.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub auction_id: i64,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitPrices {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy: U256,
}
