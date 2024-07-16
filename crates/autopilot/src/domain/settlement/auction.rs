//! Auction data related to the specific settlement.

use {
    crate::domain::{self},
    std::collections::HashMap,
};

#[derive(Debug)]
pub struct Auction {
    pub id: domain::auction::Id,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// All orders from a competition auction. Some of them may contain fee
    /// policies.
    pub orders: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
    /// Deadline for an auction solution to be settled, so that it is eligible
    /// for rewards.
    pub deadline: domain::eth::BlockNo,
    /// JIT orders with surplus capturing JIT order owners should capture
    /// surplus if settled.
    pub surplus_capturing_jit_order_owners: Vec<domain::eth::Address>,
}
