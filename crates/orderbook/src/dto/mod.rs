pub mod auction;
pub mod order;

use {
    alloy::primitives::U256,
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    simulator::tenderly,
};
pub use {
    auction::{Auction, AuctionId, AuctionWithId},
    order::Order,
};

/// The result of Order simulation, contains the error (if any)
/// and full Tenderly API request that can be used to resimulate
/// and debug using Tenderly
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct OrderSimulation {
    pub tenderly_request: tenderly::dto::Request,
    pub error: Option<String>,
}

#[serde_as]
#[derive(Serialize)]
#[cfg_attr(feature = "e2e", derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct TokenMetadata {
    pub first_trade_block: Option<u32>,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub native_price: Option<U256>,
}
