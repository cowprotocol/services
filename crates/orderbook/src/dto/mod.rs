pub mod auction;
pub mod order;

pub use {
    auction::{Auction, AuctionId, AuctionWithId},
    order::Order,
};
use {serde::Serialize, serde_with::serde_as};

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMetadata {
    pub first_trade_block: Option<u32>,
}
