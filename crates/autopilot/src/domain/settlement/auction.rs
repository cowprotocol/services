//! Auction data related to the specific settlement.

use {
    crate::domain::{self},
    serde::Deserialize,
    std::collections::{HashMap, HashSet},
};

/// Auction data that is relevant for processing a specific onchain
/// transaction. e.g. `prices` will contain only prices for tokens
/// that were actually traded in the transaction - not ALL the prices
/// that were provided in the original auction.
#[derive(Debug, Deserialize)]
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
