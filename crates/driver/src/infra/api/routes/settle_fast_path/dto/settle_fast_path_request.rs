use {
    crate::infra::api::routes::solve::dto::solve_request::Order,
    eth_domain_types as eth,
    serde::Deserialize,
    serde_with::serde_as,
};

/// Settle the quote solution cached under `quote_id`, re-encoded against the
/// real signed order, outside of a competition.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleFastPathRequest {
    /// Id of the quote whose cached solution should be settled.
    pub quote_id: i64,
    /// The real signed order the cached solution is re-encoded against.
    pub order: Order,
    /// The sell/buy amounts defining the exact price the order must fill at.
    pub limit_prices: LimitPrices,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID the autopilot allocated for this settlement. The quote was
    /// solved outside of any auction, but its settlement is still attributed
    /// to one.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub auction_id: i64,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitPrices {
    #[serde_as(as = "serde_ext::U256")]
    pub sell: eth::U256,
    #[serde_as(as = "serde_ext::U256")]
    pub buy: eth::U256,
}
