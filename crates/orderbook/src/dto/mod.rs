pub mod auction;
pub mod order;

use {
    alloy::primitives::U256,
    eth_domain_types::Address,
    model::order::{BuyTokenDestination, OrderKind, SellTokenSource},
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    simulator::tenderly,
};
pub use {
    auction::{Auction, AuctionId, AuctionWithId},
    order::Order,
};

/// Request body for the POST /api/v1/debug/simulation endpoint.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderSimulationRequest {
    pub sell_token: Address,
    pub buy_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: alloy::primitives::U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: alloy::primitives::U256,
    pub kind: OrderKind,
    pub owner: Address,
    #[serde(default)]
    pub receiver: Option<Address>,
    #[serde(default)]
    pub sell_token_balance: SellTokenSource,
    #[serde(default)]
    pub buy_token_balance: BuyTokenDestination,
    /// Full app data JSON. Defaults to `"{}"` if omitted.
    #[serde(default)]
    pub app_data: Option<String>,
    #[serde(default)]
    pub block_number: Option<u64>,
}

/// The result of Order simulation, contains the error (if any)
/// and full Tenderly API request that can be used to resimulate
/// and debug using Tenderly
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct OrderSimulationResult {
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
