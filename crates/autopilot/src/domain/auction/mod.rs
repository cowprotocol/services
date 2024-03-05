use {
    super::Order,
    primitive_types::{H160, U256},
    std::collections::BTreeMap,
};

pub mod order;

/// Replicates [`crate::model::Auction`].
#[derive(Clone, Debug, PartialEq)]
pub struct Auction {
    pub block: u64,
    pub latest_settlement_block: u64,
    pub orders: Vec<Order>,
    pub prices: BTreeMap<H160, U256>,
}

pub type Id = i64;

#[derive(Clone, Debug)]
pub struct AuctionWithId {
    pub id: Id,
    pub auction: Auction,
}
