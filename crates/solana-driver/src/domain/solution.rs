//! Domain model of a solver engine's solution.

use {
    super::order_uid::OrderUid,
    solana_sdk::{instruction::Instruction, pubkey::Pubkey},
    std::{collections::HashMap, num::NonZero},
};

/// A single solver engine's response to one auction.
#[derive(Clone, Debug)]
pub struct Solution {
    /// Id assigned by the engine in its `/solve` response. Unique within one
    /// auction, but since engines chooses the numbering, ids may repeat across
    /// auctions. The driver deduplicates repeated ids within one response,
    /// keeping only the first occurrence.
    pub id: u64,
    /// The on-chain identity of the solver that produced this solution.
    pub solver: Pubkey,
    /// Uniform clearing prices by mint: the sell mint maps to the amount
    /// bought and the buy mint to the amount sold, describing the engine's fill
    /// before the solver fee. For every trade the engine reported
    /// `executed_sell * price_sell == executed_buy * price_buy`. Once the
    /// solver fee is applied (in `compute_solutions`), the trade legs are
    /// post-fee and no longer satisfy that identity. Nothing reads `prices`
    /// after conversion.
    pub prices: HashMap<Pubkey, NonZero<u64>>,
    pub trades: Vec<Trade>,
    /// Solana instructions to execute as part of the settlement.
    pub interactions: Vec<Instruction>,
    /// Address lookup tables the interactions assume.
    pub address_lookup_tables: Vec<Pubkey>,
    /// Optional solver estimate of total settlement compute units.
    pub cu_estimate: Option<u32>,
    pub transaction_version: TransactionVersion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionVersion {
    V0,
    V1,
}

/// A fulfillment of one auction order.
#[derive(Clone, Debug)]
pub struct Trade {
    pub order_uid: OrderUid,
    /// Sell-token units executed.
    pub executed_sell: u64,
    /// Buy-token units executed.
    pub executed_buy: u64,
    /// Solver fee units retained, in the buy mint for a sell order and the
    /// sell mint for a buy order. Zero when the solver charges no fee.
    pub solver_fee: u64,
}
