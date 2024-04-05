//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! we define the way to calculate the protocol fee based on the configuration
//! parameters.

mod policy;

use {
    crate::{
        arguments::{self},
        boundary::{self},
        domain::{self},
    },
    app_data::Validator,
    derive_more::Into,
    itertools::Itertools,
    primitive_types::{H160, U256},
    prometheus::core::Number,
    std::{collections::HashSet, str::FromStr},
};

enum OrderClass {
    Market,
    Limit,
    Any,
}

impl From<arguments::FeePolicyOrderClass> for OrderClass {
    fn from(value: arguments::FeePolicyOrderClass) -> Self {
        match value {
            arguments::FeePolicyOrderClass::Market => Self::Market,
            arguments::FeePolicyOrderClass::Limit => Self::Limit,
            arguments::FeePolicyOrderClass::Any => Self::Any,
        }
    }
}

/// Constructs fee policies based on the current configuration.
pub struct ProtocolFee {
    policy: policy::Policy,
    order_class: OrderClass,
}

impl From<arguments::FeePolicy> for ProtocolFee {
    fn from(value: arguments::FeePolicy) -> Self {
        Self {
            policy: value.fee_policy_kind.into(),
            order_class: value.fee_policy_order_class.into(),
        }
    }
}

pub type ProtocolFeeExemptAddresses = HashSet<H160>;

pub struct ProtocolFees {
    fee_policies: Vec<ProtocolFee>,
    max_partner_fee: FeeFactor,
    /// List of addresses which are exempt from the protocol
    /// fees
    protocol_fee_exempt_addresses: ProtocolFeeExemptAddresses,
}

impl ProtocolFees {
    pub fn new(
        fee_policies: &[arguments::FeePolicy],
        fee_policy_max_partner_fee: FeeFactor,
        protocol_fee_exempt_addresses: &[H160],
    ) -> Self {
        Self {
            fee_policies: fee_policies
                .iter()
                .cloned()
                .map(ProtocolFee::from)
                .collect(),
            protocol_fee_exempt_addresses: protocol_fee_exempt_addresses
                .iter()
                .cloned()
                .collect::<HashSet<_>>(),
            max_partner_fee: fee_policy_max_partner_fee,
        }
    }

    /// Converts an order from the boundary layer to the domain layer, applying
    /// protocol fees if necessary.
    pub fn apply(&self, order: boundary::Order, quote: &domain::Quote) -> domain::Order {
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
                    factor: FeeFactor::try_from_capped(
                        partner_fee.bps.into_f64() / 10_000.0,
                        self.max_partner_fee.into(),
                    )
                    .unwrap(),
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
        let protocol_fees = if self
            .protocol_fee_exempt_addresses
            .contains(&order.metadata.owner)
        {
            vec![]
        } else {
            self
                    .fee_policies
                    .iter()
                    // TODO: support multiple fee policies
                    .find_map(|fee_policy| {
                        let outside_market_price = boundary::is_order_outside_market_price(&order_, &quote_, order.data.kind);
                        match (outside_market_price, &fee_policy.order_class) {
                            (_, OrderClass::Any) => Some(&fee_policy.policy),
                            (true, OrderClass::Limit) => Some(&fee_policy.policy),
                            (false, OrderClass::Market) => Some(&fee_policy.policy),
                            _ => None,
                        }
                    })
                    .and_then(|policy| match policy {
                        policy::Policy::Surplus(variant) => variant.apply(&order),
                        policy::Policy::PriceImprovement(variant) => variant.apply(&order, quote),
                        policy::Policy::Volume(variant) => variant.apply(&order),
                    })
                    .into_iter()
                    .collect_vec()
        };
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
        factor: FeeFactor,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: FeeFactor,
    },
    /// A price improvement corresponds to a situation where the order is
    /// executed at a better price than the top quote. The protocol fee in such
    /// case is calculated from a cut of this price improvement.
    PriceImprovement {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
        quote: Quote,
    },
    /// How much of the order's volume should be taken as a protocol fee.
    /// The fee is taken in `sell` token for `sell` orders and in `buy`
    /// token for `buy` orders.
    Volume {
        /// Percentage of the order's volume should be taken as a protocol
        /// fee.
        factor: FeeFactor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Into)]
pub struct FeeFactor(f64);

impl FeeFactor {
    /// Convert a fee into a `FeeFactor` capping its value
    pub fn try_from_capped(value: f64, cap: f64) -> anyhow::Result<Self> {
        value.max(0.0).min(cap).try_into()
    }
}

/// TryFrom implementation for the cases we want to enforce the constrain [0, 1)
impl TryFrom<f64> for FeeFactor {
    type Error = anyhow::Error;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            (0.0..1.0).contains(&value),
            "Factor must be in the range [0, 1)"
        );
        Ok(FeeFactor(value))
    }
}

impl FromStr for FeeFactor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<f64>().map(FeeFactor::try_from)?
    }
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
