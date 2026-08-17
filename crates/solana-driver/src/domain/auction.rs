//! Domain model of an auction the driver asks solver engines to fill.

use {super::order_uid::OrderUid, serde::Serialize, solana_sdk::pubkey::Pubkey};

/// A collection of orders the driver wants solvers to fill.
#[derive(Clone, Debug)]
pub struct Auction {
    pub id: u64,
    pub orders: Vec<Order>,
}

/// One order available for solvers to fill.
#[derive(Clone, Debug)]
pub struct Order {
    pub uid: OrderUid,
    pub sell_mint: Pubkey,
    pub buy_mint: Pubkey,
    /// Sell amount for sells, buy amount for buys.
    pub amount: u64,
    pub side: Side,
}

/// Direction of the trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Sell,
    Buy,
}
