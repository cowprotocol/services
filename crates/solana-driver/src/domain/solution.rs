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
    /// The settlement's compute unit limit: the simulated usage plus the
    /// headroom, or the solver's estimate until the simulation measures it.
    pub cu_estimate: Option<u32>,
}

/// A fulfillment of one auction order.
#[derive(Clone, Debug)]
pub struct Trade {
    pub order_uid: OrderUid,
    /// Sell-token units executed.
    pub executed_sell: u64,
    /// Buy-token units executed.
    pub executed_buy: u64,
    /// Solver fee units retained, in the buy token for a sell order and the
    /// sell token for a buy order. Zero when the solver charges no fee.
    pub solver_fee: u64,
    /// Protocol fee units taken, in the buy token for a sell order and the
    /// sell token for a buy order. Zero when the order carries no policy.
    pub protocol_fee: u64,
    /// Execution cost units cut, in the buy token for a sell order and the
    /// sell token for a buy order. Zero when the engine prices the costs.
    pub execution_cost: u64,
}

impl Trade {
    /// Units the driver took from the fill, in the buy token for a sell order
    /// and the sell token for a buy order: the execution cost, the solver fee
    /// and the protocol fee.
    pub fn fee(&self) -> u64 {
        self.execution_cost
            .saturating_add(self.solver_fee)
            .saturating_add(self.protocol_fee)
    }
}
