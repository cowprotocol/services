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
    app_data::Validator,
    itertools::Itertools,
    primitive_types::U256,
    prometheus::core::Number,
};

/// Constructs fee policies based on the current configuration.
pub enum ProtocolFee {
    Market(policy::Policy),
    Limit(policy::Policy),
}

impl ProtocolFee {
    pub fn new(fee_policy_args: arguments::FeePolicy) -> Self {
        match fee_policy_args {
            arguments::FeePolicy::Market(policy) => Self::Market(policy.into()),
            arguments::FeePolicy::Limit(policy) => Self::Limit(policy.into()),
        }
    }

    /// Converts an order from the boundary layer to the domain layer, applying
    /// protocol fees if necessary.
    pub fn apply(
        protocol_fees: &[ProtocolFee],
        order: boundary::Order,
        quote: &domain::Quote,
    ) -> domain::Order {
        // If the partner fee is specified, it overwrites the current volume fee policy
        if let Some(validated_app_data) = order
            .metadata
            .full_app_data
            .as_ref()
            .map(|full_app_data| Validator::new(usize::MAX).validate(full_app_data.as_bytes()))
            .transpose()
            .ok()
            .flatten()
        {
            if let Some(partner_fee) = validated_app_data.protocol.partner_fee {
                let fee_policy = vec![Policy::Volume {
                    factor: partner_fee.bps.into_f64() / 10_000.0,
                }];
                return boundary::order::to_domain(order, fee_policy);
            }
        }

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
        let protocol_fees = protocol_fees
            .iter()
            // TODO: support multiple fee policies
            .find_map(|fee_policy| {
                let outside_market_price = boundary::is_order_outside_market_price(&order_, &quote_);
                match (outside_market_price, fee_policy) {
                    (true, ProtocolFee::Limit(policy)) => Some(policy),
                    (false, ProtocolFee::Market(policy)) => Some(policy),
                    _ => None,
                }
            })
            .and_then(|policy| match policy {
                policy::Policy::Surplus(variant) => variant.apply(&order),
                policy::Policy::PriceImprovement(variant) => variant.apply(&order, quote),
                policy::Policy::Volume(variant) => variant.apply(&order),
            })
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
