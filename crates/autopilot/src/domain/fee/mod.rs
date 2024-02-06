//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! we define the way to calculate the protocol fee based on the configuration
//! parameters.

use crate::{
    boundary::{self},
    domain,
};

/// Constructs fee policies based on the current configuration.
#[derive(Debug)]
pub struct ProtocolFee {
    policy: Policy,
    fee_policy_skip_market_orders: bool,
}

impl ProtocolFee {
    pub fn new(policy: Policy, fee_policy_skip_market_orders: bool) -> Self {
        Self {
            policy,
            fee_policy_skip_market_orders,
        }
    }

    /// Get policies for order.
    pub fn get(&self, order: &boundary::Order, quote: Option<&domain::Quote>) -> Vec<Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => {
                if self.fee_policy_skip_market_orders {
                    vec![]
                } else {
                    vec![self.policy]
                }
            }
            boundary::OrderClass::Liquidity => vec![],
            boundary::OrderClass::Limit => {
                if !self.fee_policy_skip_market_orders {
                    return vec![self.policy];
                }

                // if the quote is missing, we can't determine if the order is outside the
                // market price so we protect the user and not charge a fee
                let Some(quote) = quote else {
                    return vec![];
                };

                let order_ = boundary::Amounts {
                    sell: order.data.sell_amount,
                    buy: order.data.buy_amount,
                    fee: order.data.fee_amount,
                };
                let quote = boundary::Amounts {
                    sell: quote.sell_amount,
                    buy: quote.buy_amount,
                    fee: quote.fee,
                };
                if boundary::is_order_outside_market_price(&order_, &quote) {
                    vec![self.policy]
                } else {
                    vec![]
                }
            }
        }
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
    /// How much of the order's volume should be taken as a protocol fee.
    /// The fee is taken in `sell` token for `sell` orders and in `buy`
    /// token for `buy` orders.
    Volume {
        /// Percentage of the order's volume should be taken as a protocol
        /// fee.
        factor: f64,
    },
}
