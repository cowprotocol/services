//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! we define the way to calculate the protocol fee based on the configuration
//! parameters.

mod policy;

use {
    crate::{
        arguments,
        boundary::{self},
        domain,
    },
    itertools::Itertools,
    primitive_types::U256,
};

/// Constructs fee policies based on the current configuration.
pub struct ProtocolFee {
    policy: policy::Policy,
}

impl ProtocolFee {
    pub fn new(fee_policy_args: arguments::FeePolicy) -> Self {
        Self {
            policy: fee_policy_args.into(),
        }
    }

    /// Converts an order from the boundary layer to the domain layer, applying
    /// protocol fees if necessary.
    pub fn apply(&self, order: boundary::Order, quote: &domain::Quote) -> domain::Order {
        let protocol_fees = match &self.policy {
            policy::Policy::Surplus(variant) => variant.apply(&order, quote),
            policy::Policy::PriceImprovement(variant) => variant.apply(&order, quote),
            policy::Policy::Volume(variant) => variant.apply(&order),
        }
        .into_iter()
        .collect_vec();
        boundary::order::to_domain(order, protocol_fees)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Policy {
    /// If the order receives more than limit price, take the protocol fee as a
    /// percentage of the difference. The fee is taken in `sell` token for
    /// `buy` orders and in `buy` token for `sell` orders.
    Surplus {
        /// Factor of surplus the protocol charges as a fee.
        /// Surplus is the difference between executed price and limit price
        ///
        /// E.g. if a user received 2000USDC for 1ETH while having a limit price
        /// of 1990USDC, their surplus is 10USDC. A factor of 0.5
        /// requires the solver to pay 5USDC to the protocol for
        /// settling this order.
        factor: f64,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: f64,
    },
    /// A price improvement corresponds to a situation where the order is
    /// executed at a better price than the top quote. The protocol fee in such
    /// case is calculated from a cut of this price improvement.
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote,
    },
    /// How much of the order's volume should be taken as a protocol fee.
    /// The fee is taken in `sell` token for `sell` orders and in `buy`
    /// token for `buy` orders.
    Volume {
        /// Percentage of the order's volume should be taken as a protocol
        /// fee.
        factor: f64,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Quote {
    /// The amount of the sell token.
    pub sell_amount: U256,
    /// The amount of the buy token.
    pub buy_amount: U256,
    /// The amount that needs to be paid, denominated in the sell token.
    pub fee: U256,
}

impl From<domain::Quote> for Quote {
    fn from(value: domain::Quote) -> Self {
        Self {
            sell_amount: value.sell_amount,
            buy_amount: value.buy_amount,
            fee: value.fee,
        }
    }
}
