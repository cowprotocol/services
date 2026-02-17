use {
    chrono::{DateTime, Utc},
    serde::{Deserialize, Serialize},
    shared::fee_factor::FeeFactor,
};

pub fn default_max_partner_fee() -> FeeFactor {
    FeeFactor::new(0.01)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FeePoliciesConfig {
    /// Describes how the protocol fees should be calculated.
    #[serde(default)]
    pub fee_policies: Vec<FeePolicy>,

    /// Maximum partner fee allowed. If the partner fee specified is greater
    /// than this maximum, the partner fee will be capped.
    #[serde(default = "default_max_partner_fee")]
    pub fee_policy_max_partner_fee: FeeFactor,

    /// Fee policies that will become effective at a future timestamp.
    #[serde(default)]
    pub upcoming_fee_policies: UpcomingFeePolicies,
}

impl Default for FeePoliciesConfig {
    fn default() -> Self {
        Self {
            fee_policies: Default::default(),
            fee_policy_max_partner_fee: default_max_partner_fee(),
            upcoming_fee_policies: Default::default(),
        }
    }
}

/// A fee policy to be used for orders based on its class.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FeePolicy {
    pub kind: FeePolicyKind,
    pub order_class: FeePolicyOrderClass,
}

/// Fee policies that will become effective at a future timestamp.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct UpcomingFeePolicies {
    #[serde(default)]
    pub fee_policies: Vec<FeePolicy>,

    pub effective_from_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeePolicyKind {
    /// How much of the order's surplus should be taken as a protocol fee.
    #[serde(rename_all = "kebab-case")]
    Surplus {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
    },
    /// How much of the order's price improvement should be taken as a protocol
    /// fee where price improvement is a difference between the executed price
    /// and the best quote.
    #[serde(rename_all = "kebab-case")]
    PriceImprovement {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
    },
    /// How much of the order's volume should be taken as a protocol fee.
    Volume { factor: FeeFactor },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeePolicyOrderClass {
    /// If a fee policy needs to be applied to in-market orders.
    Market,
    /// If a fee policy needs to be applied to limit orders.
    Limit,
    /// If a fee policy needs to be applied regardless of the order class.
    Any,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_fee_policy_surplus() {
        let toml = r#"
        kind.surplus = { factor = 0.5, max-volume-factor = 0.9 }
        order-class = "limit"
        "#;
        let policy: FeePolicy = toml::from_str(toml).unwrap();
        assert!(matches!(
            policy.kind,
            FeePolicyKind::Surplus { factor, max_volume_factor }
            if factor.get() == 0.5 && max_volume_factor.get() == 0.9
        ));
        assert!(matches!(policy.order_class, FeePolicyOrderClass::Limit));
    }

    #[test]
    fn deserialize_fee_policy_volume() {
        let toml = r#"
        kind.volume = { factor = 0.1 }
        order-class = "any"
        "#;
        let policy: FeePolicy = toml::from_str(toml).unwrap();
        assert!(matches!(
            policy.kind,
            FeePolicyKind::Volume { factor } if factor.get() == 0.1
        ));
        assert!(matches!(policy.order_class, FeePolicyOrderClass::Any));
    }

    #[test]
    fn deserialize_fee_policy_price_improvement() {
        let toml = r#"
        kind.price-improvement = { factor = 0.5, max-volume-factor = 0.06 }
        order-class = "market"
        "#;
        let policy: FeePolicy = toml::from_str(toml).unwrap();
        assert!(matches!(
            policy.kind,
            FeePolicyKind::PriceImprovement { factor, max_volume_factor }
            if factor.get() == 0.5 && max_volume_factor.get() == 0.06
        ));
        assert!(matches!(policy.order_class, FeePolicyOrderClass::Market));
    }

    #[test]
    fn deserialize_fee_policies_config_defaults() {
        let toml = "";
        let config: FeePoliciesConfig = toml::from_str(toml).unwrap();
        assert!(config.fee_policies.is_empty());
        assert_eq!(config.fee_policy_max_partner_fee.get(), 0.01);
        assert!(config.upcoming_fee_policies.fee_policies.is_empty());
        assert!(
            config
                .upcoming_fee_policies
                .effective_from_timestamp
                .is_none()
        );
    }

    #[test]
    fn deserialize_fee_policies_config_full() {
        let toml = r#"
        fee-policy-max-partner-fee = 0.005

        [[fee-policies]]
        kind.surplus = { factor = 0.5, max-volume-factor = 0.9 }
        order-class = "limit"

        [[fee-policies]]
        kind.volume = { factor = 0.1 }
        order-class = "any"

        [upcoming-fee-policies]
        effective-from-timestamp = "2025-06-01T00:00:00Z"

        [[upcoming-fee-policies.fee-policies]]
        kind.volume = { factor = 0.2 }
        order-class = "any"
        "#;
        let config: FeePoliciesConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.fee_policies.len(), 2);
        assert_eq!(config.fee_policy_max_partner_fee.get(), 0.005);
        assert_eq!(config.upcoming_fee_policies.fee_policies.len(), 1);
        assert!(
            config
                .upcoming_fee_policies
                .effective_from_timestamp
                .is_some()
        );
    }

    #[test]
    fn deserialize_invalid_fee_factor() {
        let toml = r#"
        kind.volume = { factor = 1.5 }
        order-class = "any"
        "#;
        assert!(toml::from_str::<FeePolicy>(toml).is_err());
    }
}
