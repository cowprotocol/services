pub mod auction;
pub mod order;

use {
    alloy::primitives::U256,
    eth_domain_types::{Address, NonZeroU256},
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
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct OrderSimulationRequest {
    /// The address of the token being sold.
    pub sell_token: Address,
    /// The address of the token being bought.
    pub buy_token: Address,
    /// The amount of `sell_token`s that may be sold.
    pub sell_amount: NonZeroU256,
    /// The amount of `buy_token`s that should be bought.
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    /// The kind of order (i.e. sell or buy).
    pub kind: OrderKind,
    /// The address of the order's owner
    pub owner: Address,
    /// The receiver of the `buy_token`. When this field is `None`, the receiver
    /// is the same as the owner.
    #[serde(default)]
    pub receiver: Option<Address>,
    /// Sell token's source — ERC20, internal vault or external vault (at the
    /// time of writing).
    #[serde(default)]
    pub sell_token_balance: SellTokenSource,
    /// Defines how tokens are transferred back to the user, either as an ERC-20
    /// token transfer or internal Balancer Vault transfer.
    #[serde(default)]
    pub buy_token_balance: BuyTokenDestination,
    /// Full app data JSON. Defaults to `"{}"` if omitted.
    #[serde(default)]
    pub app_data: Option<String>,
    /// The block number at which the simulation should happen
    #[serde(default)]
    pub block_number: Option<u64>,
}

/// The result of Order simulation, contains the error (if any)
/// and full Tenderly API request that can be used to resimulate
/// and debug using Tenderly
#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderSimulationResult {
    /// Full request object that can be used directly with the Tenderly API
    pub tenderly_request: tenderly::dto::Request,
    /// Shared Tenderly simulation URL for debugging in the dashboard
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenderly_url: Option<String>,
    /// Any error that might have been reported during order simulation
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
