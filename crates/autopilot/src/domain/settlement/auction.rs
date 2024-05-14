//! Auction data related to the specific settlement.

use {
    crate::domain::{self, competition, eth},
    std::collections::{HashMap, HashSet},
};

/// Offchain auction data related to a specific settlement that got settled for
/// it.
#[derive(Debug)]
pub struct Auction {
    /// Competition winner (solver submission address).
    pub winner: eth::Address,
    /// Settlement should appear onchain before this block.
    pub deadline: eth::BlockNo,
    /// Winning score promised during competition (based on the promised
    /// `competition::Solution`)
    pub score: competition::Score,
    /// Winning solution promised during competition.
    pub solution: competition::Solution,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// Settlement orders that are missing from the orderbook (JIT orders).
    pub missing_orders: HashSet<domain::OrderUid>,
    /// Fee policies for all settled orders
    pub fee_policies: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
}
