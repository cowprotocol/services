use {
    super::Order,
    primitive_types::{H160, U256},
    std::collections::BTreeMap,
};

pub mod order;

#[derive(Clone, Debug, PartialEq)]
pub struct Auction {
    /// The block that this auction is valid for.
    /// The block number for the auction. Orders and prices are guaranteed to be
    /// valid on this block.
    pub block: u64,

    /// The latest block on which a settlement has been processed. This field is
    /// used to tell which orders are still in-flight. See
    /// [`InFlightOrders`].
    ///
    /// Note that under certain conditions it is possible for a settlement to
    /// have been mined as part of [`block`] but not have yet been processed.
    pub latest_settlement_block: u64,

    /// The solvable orders included in the auction.
    pub orders: Vec<Order>,

    /// The reference prices for all traded tokens in the auction.
    pub prices: BTreeMap<H160, U256>,
}

pub type AuctionId = i64;

#[derive(Clone, Debug)]
pub struct AuctionWithId {
    pub id: AuctionId,
    pub auction: Auction,
}
