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
    alloy::primitives::{Address, U256},
    chrono::{DateTime, Utc},
    rust_decimal::Decimal,
    shared::{
        arguments::{FeeFactor, TokenBucketFeeOverride},
        fee::VolumeFeePolicy,
    },
    std::collections::HashSet,
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

pub struct UpcomingProtocolFees {
    fee_policies: Vec<ProtocolFee>,
    effective_from_timestamp: DateTime<Utc>,
}

impl From<arguments::UpcomingFeePolicies> for Option<UpcomingProtocolFees> {
    fn from(value: arguments::UpcomingFeePolicies) -> Self {
        value
            // both config fields must be non-empty
            .effective_from_timestamp
            .filter(|_| !value.fee_policies.is_empty())
            .map(|effective_from_timestamp| UpcomingProtocolFees {
                fee_policies: value
                    .fee_policies
                    .into_iter()
                    .map(ProtocolFee::from)
                    .collect::<Vec<_>>(),
                effective_from_timestamp,
            })
    }
}

pub type ProtocolFeeExemptAddresses = HashSet<Address>;

pub struct ProtocolFees {
    fee_policies: Vec<ProtocolFee>,
    max_partner_fee: FeeFactor,
    upcoming_fee_policies: Option<UpcomingProtocolFees>,
    volume_fee_policy: VolumeFeePolicy,
}

impl ProtocolFees {
    pub fn new(
        config: &arguments::FeePoliciesConfig,
        volume_fee_bucket_overrides: Vec<TokenBucketFeeOverride>,
        enable_sell_equals_buy_volume_fee: bool,
    ) -> Self {
        let volume_fee_policy = VolumeFeePolicy::new(
            volume_fee_bucket_overrides,
            None, // contained within FeePoliciesConfig; vol fee is passed in at callsite
            enable_sell_equals_buy_volume_fee,
        );
        Self {
            fee_policies: config
                .fee_policies
                .iter()
                .cloned()
                .map(ProtocolFee::from)
                .collect(),
            max_partner_fee: config.fee_policy_max_partner_fee,
            upcoming_fee_policies: config.upcoming_fee_policies.clone().into(),
            volume_fee_policy,
        }
    }

    /// Returns the capped aggregated partner fee
    fn get_partner_fee(
        order: &boundary::Order,
        quote: &domain::Quote,
        max_partner_fee: f64,
    ) -> Vec<Policy> {
        /// Number of basis points that make up 100%.
        const MAX_BPS: u32 = 10_000;

        /// Convert a fee into a `FeeFactor` capping its value
        fn fee_factor_from_capped(
            value: Decimal,
            cap: Decimal,
            accumulated: &mut Decimal,
        ) -> FeeFactor {
            // Calculate how much more we can compound before hitting the cap.
            //
            // When dealing with fee factors or percentages in compounding operations:
            // - We use (1 + x) where x is the percentage as a decimal (e.g., 5% = 0.05 →
            //   1.05)
            // - This is because applying a fee means multiplying by (1 + fee_rate)
            //
            // The total accumulated factor can't exceed (1 + cap), and we've
            // already accumulated to (1 + accumulated), then:
            //
            // 1. Current value with accumulated fees: (1 + accumulated)
            // 2. Maximum allowed value: (1 + cap)
            // 3. To find the remaining factor we can apply: (1 + cap) / (1 + accumulated) -
            //    1
            //
            // The subtraction of 1 at the end converts back from the multiplier form (1.xx)
            // to the percentage form (0.xx) that our FeeFactor expects.
            let remaining_factor =
                (Decimal::ONE + cap) / (Decimal::ONE + *accumulated) - Decimal::ONE;

            // update the `accumulated` value
            *accumulated += value.min(cap - *accumulated);

            FeeFactor::new(f64::try_from(value.max(Decimal::ZERO).min(remaining_factor)).unwrap())
        }

        fn fee_factor_from_bps(bps: u64) -> FeeFactor {
            let bps = u32::try_from(bps.min(u64::from(MAX_BPS) - 1))
                .expect("value was clamped to range expected by FeeFactor: [0, 1)");
            let factor = f64::from(bps) / f64::from(MAX_BPS);
            FeeFactor::try_from(factor).expect("value was clamped to the required range")
        }

        let Ok(max_partner_fee) = Decimal::try_from(max_partner_fee) else {
            return vec![];
        };
        let Some(full_app_data) = order.metadata.full_app_data.as_ref() else {
            return vec![];
        };
        let Ok(parsed_app_data) = app_data::parse(full_app_data.as_bytes()) else {
            return vec![];
        };

        let mut accumulated = Decimal::ZERO;

        parsed_app_data
            .partner_fee
            .iter()
            .map(move |partner_fee| {
                match partner_fee.policy {
                    app_data::FeePolicy::Volume { bps } => {
                        // Convert bps to decimal percentage
                        let fee_decimal = Decimal::from(bps) / Decimal::from(MAX_BPS);
                        // Create policy and update accumulator
                        let factor =
                            fee_factor_from_capped(fee_decimal, max_partner_fee, &mut accumulated);
                        Policy::Volume { factor }
                    }
                    app_data::FeePolicy::Surplus {
                        bps,
                        max_volume_bps,
                    } => {
                        // Convert bps to decimal percentage
                        let fee_decimal = Decimal::from(max_volume_bps) / Decimal::from(MAX_BPS);

                        // Compute max_volume_factor limited by the global volume cap.
                        let max_volume_factor =
                            fee_factor_from_capped(fee_decimal, max_partner_fee, &mut accumulated);

                        let factor = fee_factor_from_bps(bps);

                        Policy::Surplus {
                            factor,
                            max_volume_factor,
                        }
                    }
                    app_data::FeePolicy::PriceImprovement {
                        bps,
                        max_volume_bps,
                    } => {
                        // Convert bps to decimal percentage
                        let fee_decimal = Decimal::from(max_volume_bps) / Decimal::from(MAX_BPS);

                        // Compute max_volume_factor limited by the global volume cap.
                        let max_volume_factor =
                            fee_factor_from_capped(fee_decimal, max_partner_fee, &mut accumulated);

                        let factor = fee_factor_from_bps(bps);

                        Policy::PriceImprovement {
                            factor,
                            max_volume_factor,
                            quote: Quote {
                                sell_amount: quote.sell_amount.0,
                                buy_amount: quote.buy_amount.0,
                                fee: quote.fee.0,
                                solver: quote.solver,
                            },
                        }
                    }
                }
            })
            .collect::<Vec<_>>()
    }

    /// Converts an order from the boundary layer to the domain layer, applying
    /// protocol fees if necessary.
    pub fn apply(
        &self,
        order: boundary::Order,
        quote: Option<domain::Quote>,
        surplus_capturing_jit_order_owners: &[eth::Address],
    ) -> domain::Order {
        // In case there is no quote, we assume 0 buy amount so that the order ends up
        // being considered out of market price.
        let reference_quote = quote.clone().unwrap_or(domain::Quote {
            order_uid: order.metadata.uid.into(),
            sell_amount: order.data.sell_amount.into(),
            buy_amount: U256::ZERO.into(),
            fee: order.data.fee_amount.into(),
            solver: Address::ZERO,
        });

        let partner_fee =
            Self::get_partner_fee(&order, &reference_quote, self.max_partner_fee.get());

        if surplus_capturing_jit_order_owners.contains(&order.metadata.owner) {
            return boundary::order::to_domain(order, partner_fee, quote);
        }

        self.apply_policies(order, reference_quote, partner_fee)
    }

    fn apply_policies(
        &self,
        order: boundary::Order,
        quote: domain::Quote,
        partner_fees: Vec<Policy>,
    ) -> domain::Order {
        let now = Utc::now();
        let fee_policies = self
            .upcoming_fee_policies
            .as_ref()
            .filter(|upcoming| upcoming.effective_from_timestamp <= now)
            .map(|upcoming| &upcoming.fee_policies)
            .unwrap_or(&self.fee_policies);

        let protocol_fees = fee_policies
            .iter()
            .filter_map(|fee_policy| Self::protocol_fee_into_policy(&order, &quote, fee_policy))
            .flat_map(|policy| self.variant_fee_apply(&order, &quote, policy))
            .chain(partner_fees)
            .collect::<Vec<_>>();

        boundary::order::to_domain(order, protocol_fees, Some(quote))
    }

    fn variant_fee_apply(
        &self,
        order: &boundary::Order,
        quote: &domain::Quote,
        policy: &policy::Policy,
    ) -> Option<Policy> {
        match policy {
            policy::Policy::Surplus(variant) => variant.apply(order),
            policy::Policy::PriceImprovement(variant) => variant.apply(order, quote),
            policy::Policy::Volume(variant) => variant.apply(order, &self.volume_fee_policy),
        }
    }

    fn protocol_fee_into_policy<'a>(
        order: &boundary::Order,
        quote: &domain::Quote,
        protocol_fee: &'a ProtocolFee,
    ) -> Option<&'a policy::Policy> {
        let outside_market_price =
            boundary::is_order_outside_market_price(&order.into(), &quote.into(), order.data.kind);
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Quote {
    /// The amount of the sell token.
    pub sell_amount: U256,
    /// The amount of the buy token.
    pub buy_amount: U256,
    /// The amount that needs to be paid, denominated in the sell token.
    pub fee: U256,
    pub solver: Address,
}

impl Quote {
    fn from_domain(value: &domain::Quote) -> Self {
        Self {
            sell_amount: value.sell_amount.0,
            buy_amount: value.buy_amount.0,
            fee: value.fee.0,
            solver: value.solver,
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, model::order::OrderMetadata};

    #[test]
    fn test_get_partner_fee_valid_multiple_fees_not_capped() {
        // Scenario: Multiple partner fees, with valid values (not capped)
        let order = boundary::Order {
            metadata: OrderMetadata {
                full_app_data: Some(
                    r#"
                {
                    "appCode": "CoW Swap",
                    "environment": "production",
                    "metadata": {
                        "partnerFee": [
                            {
                                "bps": 500,
                                "recipient": "0x0202020202020202020202020202020202020202"
                            },
                            {
                                "bps": 2000,
                                "recipient": "0x0101010101010101010101010101010101010101"
                            }
                        ]
                    },
                    "version": "0.9.0"
                }
            "#
                    .to_string(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let max_partner_fee = 0.3; // 30%
        let result = ProtocolFees::get_partner_fee(&order, &Default::default(), max_partner_fee);

        // Expected: The compounded percentage (1 + 0.05) * (1 + 0.20) - 1 = 0.26 < 0.3
        // (not capped)
        assert_eq!(
            result,
            vec![
                Policy::Volume {
                    factor: FeeFactor::try_from(0.05).unwrap(),
                },
                Policy::Volume {
                    factor: FeeFactor::try_from(0.2).unwrap(),
                }
            ]
        );
    }

    #[test]
    fn test_get_partner_fee_empty() {
        // Scenario: No partner fees in the app data
        let order = boundary::Order {
            metadata: OrderMetadata {
                full_app_data: Some(
                    r#"
            {
                "appCode": "CoW Swap",
                "environment": "production",
                "metadata": {
                    "partnerFee": []
                },
                "version": "0.9.0"
            }
        "#
                    .to_string(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let max_partner_fee = 0.3; // 30%
        let result = ProtocolFees::get_partner_fee(&order, &Default::default(), max_partner_fee);

        // Expected: Empty vector since there are no partner fees
        assert_eq!(result, vec![]);
    }

    #[test]
    fn test_get_partner_fee_zero_bps() {
        // Scenario: Partner fee with 0 bps should be filtered out
        let order = boundary::Order {
            metadata: OrderMetadata {
                full_app_data: Some(
                    r#"
            {
                "appCode": "CoW Swap",
                "environment": "production",
                "metadata": {
                    "partnerFee": [
                        {
                            "bps": 0,
                            "recipient": "0x0202020202020202020202020202020202020202"
                        }
                    ]
                },
                "version": "0.9.0"
            }
        "#
                    .to_string(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let max_partner_fee = 0.3; // 30%
        let result = ProtocolFees::get_partner_fee(&order, &Default::default(), max_partner_fee);

        // Expected: Empty vector since the only fee has 0 bps
        assert_eq!(
            result,
            vec![Policy::Volume {
                factor: FeeFactor::try_from(0.0).unwrap(),
            }]
        );
    }

    #[test]
    fn test_get_partner_fee_zero_cap() {
        // Scenario: Partner fees with zero cap
        let order = boundary::Order {
            metadata: OrderMetadata {
                full_app_data: Some(
                    r#"
            {
                "appCode": "CoW Swap",
                "environment": "production",
                "metadata": {
                    "partnerFee": [
                        {
                            "bps": 1000,
                            "recipient": "0x0202020202020202020202020202020202020202"
                        },
                        {
                            "bps": 2000,
                            "recipient": "0x0101010101010101010101010101010101010101"
                        }
                    ]
                },
                "version": "0.9.0"
            }
        "#
                    .to_string(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let max_partner_fee = 0.0; // 0%
        let result = ProtocolFees::get_partner_fee(&order, &Default::default(), max_partner_fee);

        // Expected: All fees are capped to zero but still appear
        assert_eq!(
            result,
            vec![
                Policy::Volume {
                    factor: FeeFactor::try_from(0.0).unwrap(),
                },
                Policy::Volume {
                    factor: FeeFactor::try_from(0.0).unwrap(),
                }
            ]
        );
    }

    #[test]
    fn test_get_partner_fee_single_capped() {
        // Scenario: Single partner fee exceeding the cap
        let order = boundary::Order {
            metadata: OrderMetadata {
                full_app_data: Some(
                    r#"
            {
                "appCode": "CoW Swap",
                "environment": "production",
                "metadata": {
                    "partnerFee": [
                        {
                            "bps": 5000,
                            "recipient": "0x0202020202020202020202020202020202020202"
                        }
                    ]
                },
                "version": "0.9.0"
            }
        "#
                    .to_string(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let max_partner_fee = 0.3; // 30%
        let result = ProtocolFees::get_partner_fee(&order, &Default::default(), max_partner_fee);

        // Expected: Single fee capped at 0.3 (instead of 0.5)
        assert_eq!(
            result,
            vec![Policy::Volume {
                factor: FeeFactor::try_from(0.3).unwrap(),
            }]
        );
    }

    #[test]
    fn test_get_two_partner_fees_capped() {
        // Scenario: One partner fee gets partially capped due to compounding
        let order = boundary::Order {
            metadata: OrderMetadata {
                full_app_data: Some(
                    r#"
            {
                "appCode": "CoW Swap",
                "environment": "production",
                "metadata": {
                    "partnerFee": [
                        {
                            "bps": 1000,
                            "recipient": "0x0202020202020202020202020202020202020202"
                        },
                        {
                            "bps": 2500,
                            "recipient": "0x0101010101010101010101010101010101010101"
                        }
                    ]
                },
                "version": "0.9.0"
            }
        "#
                    .to_string(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let max_partner_fee = 0.3; // 30%
        let result = ProtocolFees::get_partner_fee(&order, &Default::default(), max_partner_fee);

        // Expected: With compounding:
        // First fee: 0.1
        // Second fee: 0.25 would result in (1+0.1)*(1+0.25)-1 = 0.375 > 0.3
        // Second fee is capped to 0.1818... to make total exactly 0.3
        assert_eq!(
            result,
            vec![
                Policy::Volume {
                    factor: FeeFactor::try_from(0.1).unwrap(),
                },
                Policy::Volume {
                    factor: FeeFactor::try_from(0.18181818181818182).unwrap(),
                }
            ]
        );
    }

    #[test]
    fn test_get_three_partner_fees_capped() {
        // Scenario: Partner fees exceeding the cap with compounding
        let order = boundary::Order {
            metadata: OrderMetadata {
                full_app_data: Some(
                    r#"
            {
                "appCode": "CoW Swap",
                "environment": "production",
                "metadata": {
                    "partnerFee": [
                        {
                            "bps": 1000,
                            "recipient": "0x0202020202020202020202020202020202020202"
                        },
                        {
                            "bps": 2000,
                            "recipient": "0x0101010101010101010101010101010101010101"
                        },
                        {
                            "bps": 1500,
                            "recipient": "0x0303030303030303030303030303030303030303"
                        }
                    ]
                },
                "version": "0.9.0"
            }
        "#
                    .to_string(),
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let max_partner_fee = 0.3; // 30%
        let result = ProtocolFees::get_partner_fee(&order, &Default::default(), max_partner_fee);

        // Expected: With compounding, fees accumulate as follows:
        // First fee: 0.1
        // Second fee: 0.2 (accumulated to this point: (1+0.1)*(1+0.2)-1 = 0.32 > 0.3)
        // Second fee gets capped to 0.1818... to make total exactly 0.3
        // Third fee: Capped to 0 since we already hit the cap
        assert_eq!(
            result,
            vec![
                Policy::Volume {
                    factor: FeeFactor::try_from(0.1).unwrap(),
                },
                Policy::Volume {
                    factor: FeeFactor::try_from(0.18181818181818182).unwrap(),
                },
                Policy::Volume {
                    factor: FeeFactor::try_from(0.0).unwrap(),
                }
            ]
        );
    }
}
