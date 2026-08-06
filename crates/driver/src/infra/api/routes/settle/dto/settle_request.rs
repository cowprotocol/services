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
    /// Real signed order for fast-path settlements. When present, the cached
    /// quote solution is re-encoded against it before settling.
    #[serde(default)]
    pub order: Option<Order>,
    /// Native prices (wei per 10**18 of the token) for the order's tokens, used
    /// to size the re-encoded settlement's slippage buffer.
    #[serde_as(as = "HashMap<_, serde_ext::U256>")]
    #[serde(default)]
    pub prices: HashMap<eth::Address, eth::U256>,
}
