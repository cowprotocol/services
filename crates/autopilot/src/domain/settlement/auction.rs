//! Auction data related to the specific settlement.

use {
    crate::domain::{self},
    std::collections::HashMap,
};

#[derive(Debug)]
pub struct Auction {
    pub id: domain::auction::Id,
    /// All orders from a competition auction. Some of them may contain fee
    /// policies and quotes.
    pub fee_policies: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
    /// All quotes from a competition auction orders.
    pub quotes: HashMap<domain::OrderUid, domain::Quote>,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// JIT orders with surplus capturing JIT order owners should capture
    /// surplus if settled.
    pub surplus_capturing_jit_order_owners: Vec<domain::eth::Address>,
}

impl Auction {
    /// Protocol defines rules whether an order is eligible to contribute to the
    /// surplus of a settlement.
    pub fn is_surplus_capturing(&self, order: &domain::OrderUid) -> bool {
        // All orders in the auction contribute to surplus
        if self.fee_policies.contains_key(order) {
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
