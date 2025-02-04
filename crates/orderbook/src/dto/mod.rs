pub mod auction;
pub mod order;

pub use {
    auction::{Auction, AuctionId, AuctionWithId},
    order::Order,
};
use {bigdecimal::BigDecimal, serde::Serialize, serde_with::serde_as};

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMetadata {
    pub first_trade_block: Option<u32>,
    pub most_recent_native_price: Option<BigDecimal>,
}
