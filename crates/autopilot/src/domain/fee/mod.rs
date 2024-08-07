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
        domain::{self, eth},
    },
    app_data::Validator,
    derive_more::Into,
    itertools::Itertools,
    primitive_types::{H160, U256},
    prometheus::core::Number,
    std::{collections::HashSet, str::FromStr},
};

#[derive(Debug)]
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
    enable_protocol_fees: bool,
}

impl ProtocolFees {
    pub fn new(
        fee_policies: &[arguments::FeePolicy],
        fee_policy_max_partner_fee: FeeFactor,
        enable_protocol_fees: bool,
    ) -> Self {
        Self {
            fee_policies: fee_policies
                .iter()
                .cloned()
                .map(ProtocolFee::from)
                .collect(),
            max_partner_fee: fee_policy_max_partner_fee,
            enable_protocol_fees,
        }
    }

    /// Converts an order from the boundary layer to the domain layer, applying
    /// protocol fees if necessary.
    pub fn apply(
        &self,
        order: boundary::Order,
        quote: Option<domain::Quote>,
        surplus_capturing_jit_order_owners: &[eth::Address],
    ) -> domain::Order {
        let partner_fee = order
            .metadata
            .full_app_data
            .as_ref()
            .and_then(|full_app_data| {
                Validator::new(usize::MAX)
                    .validate(full_app_data.as_bytes())
                    .ok()?
                    .protocol
                    .partner_fee
                    .map(|partner_fee| Policy::Volume {
                        factor: FeeFactor::try_from_capped(
                            partner_fee.bps.into_f64() / 10_000.0,
                            self.max_partner_fee.into(),
                        )
                        .unwrap(),
                    })
            })
            .into_iter()
            .collect::<Vec<_>>();

        if surplus_capturing_jit_order_owners.contains(&order.metadata.owner.into()) {
            return boundary::order::to_domain(order, partner_fee);
        }

        let order_ = boundary::Amounts {
            sell: order.data.sell_amount,
            buy: order.data.buy_amount,
            fee: order.data.fee_amount,
        };

        // In case there is no quote, we assume 0 buy amount so that the order ends up
        // being considered out of market price.
        let quote = quote.unwrap_or(domain::Quote {
            order_uid: order.metadata.uid.into(),
            sell_amount: order.data.sell_amount.into(),
            buy_amount: U256::zero().into(),
            fee: order.data.fee_amount.into(),
        });

        let quote_ = boundary::Amounts {
            sell: quote.sell_amount.into(),
            buy: quote.buy_amount.into(),
            fee: quote.fee.into(),
        };

        if self.enable_protocol_fees {
            self.apply_multiple_policies(order, &quote, order_, quote_, partner_fee)
        } else {
            self.apply_single_policy(order, &quote, order_, quote_, partner_fee)
        }
    }

    fn apply_single_policy(
        &self,
        order: boundary::Order,
        quote: &domain::Quote,
        order_: boundary::Amounts,
        quote_: boundary::Amounts,
        partner_fees: Vec<Policy>,
    ) -> domain::Order {
        if let Some(partner_fee) = partner_fees.first() {
            return boundary::order::to_domain(order, vec![*partner_fee]);
        }
        let protocol_fees = self
            .fee_policies
            .iter()
            .find_map(|fee_policy| {
                Self::protocol_fee_into_policy(&order, &order_, &quote_, fee_policy)
            })
            .and_then(|policy| Self::variant_fee_apply(&order, quote, policy))
            .into_iter()
            .collect_vec();
        boundary::order::to_domain(order, protocol_fees)
    }

    fn apply_multiple_policies(
        &self,
        order: boundary::Order,
        quote: &domain::Quote,
        order_: boundary::Amounts,
        quote_: boundary::Amounts,
        partner_fees: Vec<Policy>,
    ) -> domain::Order {
        let protocol_fees = self
            .fee_policies
            .iter()
            .filter_map(|fee_policy| {
                Self::protocol_fee_into_policy(&order, &order_, &quote_, fee_policy)
            })
            .flat_map(|policy| Self::variant_fee_apply(&order, quote, policy))
            .chain(partner_fees)
            .collect::<Vec<_>>();
        boundary::order::to_domain(order, protocol_fees)
    }

    fn variant_fee_apply(
        order: &boundary::Order,
        quote: &domain::Quote,
        policy: &policy::Policy,
    ) -> Option<Policy> {
        match policy {
            policy::Policy::Surplus(variant) => variant.apply(order),
            policy::Policy::PriceImprovement(variant) => variant.apply(order, quote),
            policy::Policy::Volume(variant) => variant.apply(order),
        }
    }

    fn protocol_fee_into_policy<'a>(
        order: &boundary::Order,
        order_: &boundary::Amounts,
        quote_: &boundary::Amounts,
        protocol_fee: &'a ProtocolFee,
    ) -> Option<&'a policy::Policy> {
        let outside_market_price =
            boundary::is_order_outside_market_price(order_, quote_, order.data.kind);
        match (outside_market_price, &protocol_fee.order_class) {
            (_, OrderClass::Any) => Some(&protocol_fee.policy),
            (true, OrderClass::Limit) => Some(&protocol_fee.policy),
            (false, OrderClass::Market) => Some(&protocol_fee.policy),
            _ => None,
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
            sell_amount: value.sell_amount.into(),
            buy_amount: value.buy_amount.into(),
            fee: value.fee.into(),
        }
    }
}
