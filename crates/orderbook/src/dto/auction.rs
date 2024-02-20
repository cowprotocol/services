use {
    super::order::Order,
    primitive_types::H160,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

/// Replicates [`crate::model::Auction`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    pub prices: BTreeMap<H160, number::U256>,
}

pub type AuctionId = i64;

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionWithId {
    /// Increments whenever the backend updates the auction.
    pub id: AuctionId,
    #[serde(flatten)]
    pub auction: Auction,
}
