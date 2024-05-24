//! Competition data related to the specific settlement.

use {
    crate::domain::{self, competition, eth},
    std::collections::HashMap,
};

/// Offchain competition data related to a specific settlement that got settled
/// for it.
#[derive(Debug)]
pub struct Competition {
    pub auction: Auction,
    /// Winning solution promised during competition.
    pub solution: competition::Solution,
}

#[derive(Debug)]
pub struct Auction {
    pub id: domain::auction::Id,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// Settlement should appear onchain before this block.
    pub deadline: eth::BlockNo,
    /// Fee policies for all settled orders
    pub fee_policies: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
}
