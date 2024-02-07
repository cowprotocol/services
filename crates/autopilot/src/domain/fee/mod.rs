//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! we define the way to calculate the protocol fee based on the configuration
//! parameters.

use {
    crate::{
        boundary::{self},
        domain,
    },
    primitive_types::U256,
};

/// Constructs fee policies based on the current configuration.
#[derive(Debug)]
pub struct ProtocolFee {
    policy_builder: PolicyBuilder,
    skip_market_orders: bool,
}

impl ProtocolFee {
    pub fn new(policy_builder: PolicyBuilder, skip_market_orders: bool) -> Self {
        Self {
            policy_builder,
            skip_market_orders,
        }
    }

    /// Converts an order from the boundary layer to the domain layer, applying
    /// protocol fees if necessary.
    pub fn apply(&self, order: boundary::Order, quote: &domain::Quote) -> domain::Order {
        let protocol_fees = self
            .policy_builder
            .build(quote)
            .with(&order, quote, self.skip_market_orders)
            .into_iter()
            .collect();
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

impl Policy {
    pub fn with(
        self,
        order: &boundary::Order,
        quote: &domain::Quote,
        skip_market_orders: bool,
    ) -> Option<Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => {
                if skip_market_orders {
                    None
                } else {
                    Some(self)
                }
            }
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => {
                if !skip_market_orders {
                    Some(self)
                } else {
                    let order_ = boundary::Amounts {
                        sell: order.data.sell_amount,
                        buy: order.data.buy_amount,
                        fee: order.data.fee_amount,
                    };
                    let quote_ = boundary::Amounts {
                        sell: quote.sell_amount,
                        buy: quote.buy_amount,
                        fee: quote.fee,
                    };
                    if boundary::is_order_outside_market_price(&order_, &quote_) {
                        Some(self)
                    } else {
                        None
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum PolicyBuilder {
    Surplus { factor: f64, max_volume_factor: f64 },
    PriceImprovement { factor: f64, max_volume_factor: f64 },
    Volume { factor: f64 },
}

impl PolicyBuilder {
    pub fn build(&self, quote: &domain::Quote) -> Policy {
        match self {
            PolicyBuilder::Surplus {
                factor,
                max_volume_factor,
            } => Policy::Surplus {
                factor: *factor,
                max_volume_factor: *max_volume_factor,
            },
            PolicyBuilder::PriceImprovement {
                factor,
                max_volume_factor,
            } => Policy::PriceImprovement {
                factor: *factor,
                max_volume_factor: *max_volume_factor,
                quote: quote.clone().into(),
            },
            PolicyBuilder::Volume { factor } => Policy::Volume { factor: *factor },
        }
    }
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
