//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! we define the way to calculate the protocol fee based on the configuration
//! parameters.

use crate::{
    boundary::{self},
    domain,
    infra::{self},
};

/// Constructs fee policies based on the current configuration.
#[derive(Debug)]
pub struct Policies {
    pub policy: Policy,
    pub fee_policy_skip_market_orders: bool,
}

impl Policies {
    pub fn new(policy: Policy, fee_policy_skip_market_orders: bool) -> Self {
        Self {
            policy,
            fee_policy_skip_market_orders,
        }
    }

    /// Get policies for order.
    pub fn get(&self, order: &boundary::Order, quote: &domain::Quote) -> Vec<Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => vec![],
            boundary::OrderClass::Liquidity => vec![],
            boundary::OrderClass::Limit(_) => {
                let is_market_order = !boundary::is_order_outside_market_price(
                    &order.data.sell_amount,
                    &order.data.buy_amount,
                    &quote.buy_amount,
                    &quote.sell_amount,
                );
                if self.fee_policy_skip_market_orders && is_market_order {
                    return vec![];
                }
                vec![self.policy]
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Policy {
    /// If the order receives more than expected (positive deviation from
    /// quoted amounts) pay the protocol a factor of the achieved
    /// improvement. The fee is taken in `sell` token for `buy`
    /// orders and in `buy` token for `sell` orders.
    PriceImprovement {
        /// Factor of price improvement the protocol charges as a fee.
        /// Price improvement is the difference between executed price and
        /// limit price or quoted price (whichever is better)
        ///
        /// E.g. if a user received 2000USDC for 1ETH while having been
        /// quoted 1990USDC, their price improvement is 10USDC.
        /// A factor of 0.5 requires the solver to pay 5USDC to
        /// the protocol for settling this order.
        factor: f64,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: f64,
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read quotes from the database")]
    Db(#[from] infra::database::quotes::Error),
}
