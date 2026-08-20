//! Domain model of a solver engine's solution.

use {
    super::order_uid::OrderUid,
    solana_sdk::{instruction::Instruction, pubkey::Pubkey},
};

/// A single solver engine's response to one auction.
#[derive(Clone, Debug)]
pub struct Solution {
    pub id: u64,
    /// The on-chain identity of the solver that produced this solution.
    pub solver: Pubkey,
    pub trades: Vec<Trade>,
    /// Solana instructions to execute as part of the settlement.
    pub interactions: Vec<Instruction>,
    /// Address lookup tables the interactions assume.
    pub address_lookup_tables: Vec<Pubkey>,
    /// Optional solver estimate of total settlement compute units.
    pub cu_estimate: Option<u64>,
}

/// A fulfillment of one auction order.
#[derive(Clone, Debug)]
pub struct Trade {
    pub order_uid: OrderUid,
    /// Sell-token units for sell orders, buy-token units for buy orders.
    pub executed_amount: u64,
}
