use {
    super::order::Order,
    alloy::primitives::{Address, U256},
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

/// Replicates [`crate::model::Auction`].
// NOTE: as of 25/11/2025 this is only used for liveness checking.
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub block: u64,
    pub orders: Vec<Order>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<Address, U256>,
    #[serde(default)]
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

pub type AuctionId = i64;

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionWithId {
    /// Increments whenever the backend updates the auction.
    pub id: AuctionId,
    #[serde(flatten)]
    pub auction: Auction,
}
