//! Minimal solution and order data structures.
//!
//! These structs contain only the data needed for winner selection,
//! making them small enough to efficiently send to/from the Pod Service.

use {
    crate::primitives::{OrderUid, Price, Side, TokenAddress, TokenAmount},
    alloy::primitives::Address,
    std::collections::HashMap,
};

/// Minimal solution data needed for winner selection.
///
/// This contains only what's absolutely necessary to run the winner selection
/// algorithm. Autopilot and driver convert their full solution types to this
/// minimal format before sending to the Pod Service.
///
/// Estimated size: ~1.7KB for a solution with 5 orders and 10 unique tokens.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Solution {
    /// Solution ID from solver (unique per solver).
    pub id: u64,

    /// Solver's submission address (used for identifying the solver).
    pub solver: Address,

    /// Orders executed in this solution.
    ///
    /// Uses Vec instead of HashMap for smaller serialization size.
    pub orders: Vec<Order>,

    /// Uniform clearing prices for all tokens in the solution.
    ///
    /// Maps token address to its price in the native token (ETH/XDAI).
    /// These are the prices at which all orders trading these tokens are
    /// settled.
    pub prices: HashMap<TokenAddress, Price>,
}

/// Minimal order data needed for winner selection.
///
/// Contains the essential information about how an order was executed,
/// including limit amounts (from the original order) and executed amounts
/// (what actually happened in this solution).
///
/// Estimated size: ~225 bytes per order.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Order {
    /// Unique order identifier (56 bytes).
    pub uid: OrderUid,

    /// Sell token address.
    pub sell_token: TokenAddress,

    /// Buy token address.
    pub buy_token: TokenAddress,

    /// Limit amount of sell token (from original order parameters).
    ///
    /// This is the maximum amount the user is willing to sell.
    pub sell_amount: TokenAmount,

    /// Limit amount of buy token (from original order parameters).
    ///
    /// This is the minimum amount the user wants to receive.
    pub buy_amount: TokenAmount,

    /// Amount of sell token that left the user's wallet (including fees).
    ///
    /// This is the actual executed amount in this solution.
    pub executed_sell: TokenAmount,

    /// Amount of buy token the user received (after fees).
    ///
    /// This is the actual amount the user got in this solution.
    pub executed_buy: TokenAmount,

    /// Order side (Buy or Sell).
    ///
    /// Determines how surplus is calculated.
    pub side: Side,
}
