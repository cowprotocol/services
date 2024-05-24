//! Auction data related to the specific settlement.

use {
    crate::domain::{self, eth},
    std::collections::HashMap,
};

#[derive(Debug)]
pub struct Auction {
    pub id: domain::auction::Id,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// Settlement should appear onchain before this block.
    pub deadline: eth::BlockNo,
    /// Fee policies for all orders in the auction. For some orders, there may
    /// be no fee policies.
    pub fee_policies: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
}
