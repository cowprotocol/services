//! Auction data related to the specific settlement.

use {
    crate::domain::{self},
    std::collections::{HashMap, HashSet},
};

#[derive(Debug)]
pub struct Auction {
    pub id: domain::auction::Id,
    /// All orders from a competition auction. Some of them may contain fee
    /// policies.
    pub orders: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// JIT orders with surplus capturing JIT order owners should capture
    /// surplus if settled.
    pub surplus_capturing_jit_order_owners: HashSet<domain::eth::Address>,
}
