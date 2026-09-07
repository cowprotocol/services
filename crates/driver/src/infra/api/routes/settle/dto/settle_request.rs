use {
    crate::infra::api::routes::solve::dto::solve_request::Order,
    eth_domain_types as eth,
    serde::Deserialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    /// Unique ID of the solution (per driver competition), to settle.
    pub solution_id: u64,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID in which this solution is competing.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub auction_id: i64,
    /// Fast-path (out-of-competition) inputs, present only when re-encoding a
    /// cached quote solution against the real signed order.
    #[serde(default)]
    pub fast_path: Option<FastPath>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastPath {
    /// The real signed order the cached solution is re-encoded against.
    pub order: Order,
    /// The sell/buy amounts defining the exact price the order must fill at.
    pub limit_prices: LimitPrices,
    /// Native prices (wei per 10**18) for the order's tokens.
    #[serde_as(as = "HashMap<_, serde_ext::U256>")]
    pub native_prices: HashMap<eth::Address, eth::U256>,
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
