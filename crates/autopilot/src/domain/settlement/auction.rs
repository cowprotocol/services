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

impl Auction {
    /// Protocol defines rules whether an order is eligible to contribute to the
    /// surplus of a settlement.
    pub fn is_surplus_capturing(&self, order: &domain::OrderUid, order_in_database: bool) -> bool {
        match self.classify(order, order_in_database) {
            super::order::Type::User => true,
            super::order::Type::UserOutOfAuction => false,
            super::order::Type::SurplusCapturingJit => true,
            super::order::Type::Jit => false,
        }
    }

    /// Is order a JIT order.
    pub fn is_jit(&self, order: &domain::OrderUid, order_in_database: bool) -> bool {
        match self.classify(order, order_in_database) {
            super::order::Type::User => false,
            super::order::Type::UserOutOfAuction => false,
            super::order::Type::SurplusCapturingJit => true,
            super::order::Type::Jit => true,
        }
    }

    /// Classify an order based on the auction data and existence of the order
    /// in the database.
    fn classify(&self, order: &domain::OrderUid, order_in_database: bool) -> super::order::Type {
        // All orders from the auction follow the regular user orders flow
        if self.orders.contains_key(order) {
            return super::order::Type::User;
        }
        // If not in auction, then check if it's a surplus capturing JIT order
        if self
            .surplus_capturing_jit_order_owners
            .contains(&order.owner())
        {
            return super::order::Type::SurplusCapturingJit;
        }
        // If not in auction and not a surplus capturing JIT order, then it's a JIT
        // order but it must not be in the database
        if !order_in_database {
            return super::order::Type::Jit;
        }
        // A regular user order but settled outside of the auction
        super::order::Type::UserOutOfAuction
    }
}
