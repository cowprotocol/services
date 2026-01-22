pub mod auction;
pub mod order;

use {
    alloy::primitives::U256,
    number::serialization::HexOrDecimalU256,
    serde::Serialize,
    serde_with::serde_as,
};
pub use {
    auction::{Auction, AuctionId, AuctionWithId},
    order::Order,
};

#[serde_as]
#[derive(Serialize)]
#[cfg_attr(feature = "e2e", derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct TokenMetadata {
    pub first_trade_block: Option<u32>,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub native_price: Option<U256>,
}
