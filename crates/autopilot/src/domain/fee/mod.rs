//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! we define the way to calculate the protocol fee based on the configuration
//! parameters.

use {
    crate::{
        arguments,
        boundary::{self},
        domain,
    },
    primitive_types::U256,
};

/// Constructs fee policies based on the current configuration.
#[derive(Debug)]
pub struct ProtocolFee {
    policy: arguments::FeePolicy,
    fee_policy_skip_market_orders: bool,
}

impl ProtocolFee {
    pub fn new(policy: arguments::FeePolicy, fee_policy_skip_market_orders: bool) -> Self {
        Self {
            policy,
            fee_policy_skip_market_orders,
        }
    }

    /// Get policies for order.
    pub fn get(
        &self,
        order: &boundary::Order,
        quote: Option<&domain::Quote>,
    ) -> anyhow::Result<Vec<Policy>> {
        match order.metadata.class {
            boundary::OrderClass::Market => {
                if self.fee_policy_skip_market_orders {
                    Ok(vec![])
                } else {
                    self.policy.to_domain(quote).map(|p| vec![p])
                }
            }
            boundary::OrderClass::Liquidity => Ok(vec![]),
            boundary::OrderClass::Limit => {
                if !self.fee_policy_skip_market_orders {
                    return self.policy.to_domain(quote).map(|p| vec![p]);
                }

                // if the quote is missing, we can't determine if the order is outside the
                // market price so we protect the user and not charge a fee
                let Some(quote) = quote else {
                    return Ok(vec![]);
                };

                if boundary::is_order_outside_market_price(
                    &order.data.sell_amount,
                    &order.data.buy_amount,
                    &quote.buy_amount,
                    &quote.sell_amount,
                ) {
                    self.policy.to_domain(Some(quote)).map(|p| vec![p])
                } else {
                    Ok(vec![])
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
    pub sell_amount: U256,
    pub buy_amount: U256,
}

impl From<domain::Quote> for Quote {
    fn from(value: domain::Quote) -> Self {
        Self {
            sell_amount: value.sell_amount,
            buy_amount: value.buy_amount,
        }
    }
}
