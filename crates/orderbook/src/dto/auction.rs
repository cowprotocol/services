use {
    super::order::Order,
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

/// Replicates [`crate::model::Auction`].
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub block: u64,
    pub orders: Vec<Order>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
    #[serde(default)]
    pub surplus_capturing_jit_order_owners: Vec<H160>,
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
