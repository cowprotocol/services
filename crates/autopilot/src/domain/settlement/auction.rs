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
    /// Fee policies for all orders in the auction. For some orders, there may
    /// be no fee policies.
    pub fee_policies: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
    /// Deadline for the auction to be settled.
    pub deadline: domain::eth::BlockNo,
    /// JIT orders with surplus capturing JIT order owners should capture
    /// surplus if settled.
    pub surplus_capturing_jit_order_owners: Vec<domain::eth::Address>,
}
