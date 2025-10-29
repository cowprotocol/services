//! Auction data related to the specific settlement.

use {
    crate::domain::{self},
    std::collections::{HashMap, HashSet},
};

/// This struct gets populated with data for a specific transaction
/// so it's allowed to prune state that is irrelevant for handling
/// that particular transaction (e.g. `prices` might only contain
/// prices for tokens that were traded in that transaction).
#[derive(Debug)]
pub struct Auction {
    pub id: domain::auction::Id,
    /// The block on top of which the auction was created.
    pub block: domain::eth::BlockNo,
    /// All orders from a competition auction. Some of them may contain fee
    /// policies.
    pub orders: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// JIT orders with surplus capturing JIT order owners should capture
    /// surplus if settled.
    pub surplus_capturing_jit_order_owners: HashSet<domain::eth::Address>,
}
