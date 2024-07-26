//! Auction data related to the specific settlement.

use {
    crate::domain::{self},
    std::collections::HashMap,
};

#[derive(Debug)]
pub struct Auction {
    pub id: domain::auction::Id,
    /// All orders from a competition auction. Some of them may contain fee
    /// policies.
    pub orders: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// Deadline for an auction solution to be settled, so that it is eligible
    /// for rewards.
    pub deadline: domain::eth::BlockNo,
    /// JIT orders with surplus capturing JIT order owners should capture
    /// surplus if settled.
    pub surplus_capturing_jit_order_owners: Vec<domain::eth::Address>,
}

impl Auction {
    /// Protocol defines rules whether an order is eligible to contribute to the
    /// surplus of a settlement.
    pub fn is_surplus_capturing(&self, order: &domain::OrderUid) -> bool {
        // All orders in the auction contribute to surplus
        if self.orders.contains_key(order) {
            return true;
        }
        // Some JIT orders contribute to surplus, for example COW AMM orders
        if self
            .surplus_capturing_jit_order_owners
            .contains(&order.owner())
        {
            return true;
        }
        false
    }
}
