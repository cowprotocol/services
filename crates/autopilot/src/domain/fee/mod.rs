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
    itertools::Itertools,
    primitive_types::U256,
};

/// Constructs fee policies based on the current configuration.
pub struct ProtocolFee {
    policy_builder: PolicyBuilder,
    skip_market_orders: bool,
}

impl ProtocolFee {
    pub fn new(policy_args: arguments::FeePolicy) -> Self {
        let skip_market_orders = policy_args.fee_policy_skip_market_orders;
        Self {
            policy_builder: policy_args.into(),
            skip_market_orders,
        }
    }

    /// Converts an order from the boundary layer to the domain layer, applying
    /// protocol fees if necessary.
    pub fn apply(&self, order: boundary::Order, quote: &domain::Quote) -> domain::Order {
        let protocol_fees = match &self.policy_builder {
            PolicyBuilder::Surplus(variant) => variant.apply(),
            PolicyBuilder::PriceImprovement(variant) => variant.apply(quote),
            PolicyBuilder::Volume(variant) => variant.apply(),
        }
        .with(&order, quote, self.skip_market_orders)
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

impl Policy {
    pub fn with(
        self,
        order: &boundary::Order,
        quote: &domain::Quote,
        skip_market_orders: bool,
    ) -> Option<Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => None,
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

enum PolicyBuilder {
    Surplus(SurplusPolicy),
    PriceImprovement(PriceImprovementPolicy),
    Volume(VolumePolicy),
}

impl From<arguments::FeePolicy> for PolicyBuilder {
    fn from(policy_arg: arguments::FeePolicy) -> Self {
        match policy_arg.fee_policy_kind {
            arguments::FeePolicyKind::Surplus {
                factor,
                max_volume_factor,
            } => PolicyBuilder::Surplus(SurplusPolicy {
                factor,
                max_volume_factor,
            }),
            arguments::FeePolicyKind::PriceImprovement {
                factor,
                max_volume_factor,
            } => PolicyBuilder::PriceImprovement(PriceImprovementPolicy {
                factor,
                max_volume_factor,
            }),
            arguments::FeePolicyKind::Volume { factor } => {
                PolicyBuilder::Volume(VolumePolicy { factor })
            }
        }
    }
}

struct SurplusPolicy {
    factor: f64,
    max_volume_factor: f64,
}

struct PriceImprovementPolicy {
    factor: f64,
    max_volume_factor: f64,
}

struct VolumePolicy {
    factor: f64,
}

impl SurplusPolicy {
    pub fn apply(&self) -> Policy {
        Policy::Surplus {
            factor: self.factor,
            max_volume_factor: self.max_volume_factor,
        }
    }
}

impl PriceImprovementPolicy {
    pub fn apply(&self, quote: &domain::Quote) -> Policy {
        Policy::PriceImprovement {
            factor: self.factor,
            max_volume_factor: self.max_volume_factor,
            quote: quote.clone().into(),
        }
    }
}

impl VolumePolicy {
    pub fn apply(&self) -> Policy {
        Policy::Volume {
            factor: self.factor,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Quote {
    pub sell_amount: U256,
    pub buy_amount: U256,
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
