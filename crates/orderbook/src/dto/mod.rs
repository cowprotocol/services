pub mod auction;
pub mod order;

pub use {
    auction::{Auction, AuctionId, AuctionWithId},
    order::Order,
};
use {
    chrono::{DateTime, Utc},
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
};

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMetadata {
    pub first_trade_block: Option<u32>,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub native_price: Option<U256>,
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementExecution {
    pub auction_id: AuctionId,
    pub solver: H160,
    pub start_timestamp: DateTime<Utc>,
    pub end_timestamp: Option<DateTime<Utc>>,
    pub start_block: u32,
    pub end_block: Option<u32>,
    pub deadline_block: u32,
    pub outcome: Option<String>,
}
