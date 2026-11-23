use {
    crate::fee_factor::FeeFactor,
    chrono::{DateTime, Utc},
    serde::{
        Deserialize,
        Deserializer,
        Serialize,
        de::{
            MapAccess,
            SeqAccess,
            Visitor,
            value::{MapAccessDeserializer, SeqAccessDeserializer},
        },
    },
    std::fmt,
};

pub const fn default_max_partner_fee() -> FeeFactor {
    FeeFactor::new(0.01)
}

/// Protocol fee configuration: which fee model to apply per order class.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FeePoliciesConfig {
    /// Describes how the protocol fees should be calculated.
    #[serde(default)]
    pub policies: Vec<FeePolicy>,

    /// Maximum partner fee allowed. If the partner fee specified is greater
    /// than this maximum, the partner fee will be capped.
    #[serde(default = "default_max_partner_fee")]
    pub max_partner_fee: FeeFactor,

    /// Fee policies that take over from `policies` at a timestamp, each entry
    /// at its own. Accepts one table or an array of tables.
    #[serde(default, deserialize_with = "one_or_many")]
    pub upcoming_policies: Vec<UpcomingFeePolicies>,
}

impl Default for FeePoliciesConfig {
    fn default() -> Self {
        Self {
            policies: Default::default(),
            max_partner_fee: default_max_partner_fee(),
            upcoming_policies: Default::default(),
        }
    }
}

/// A fee policy to be used for orders based on its class.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FeePolicy {
    /// The fee calculation method (surplus, price-improvement, or volume).
    pub kind: FeePolicyKind,
    /// Which order class this policy applies to.
    pub order_class: FeePolicyOrderClass,
}

/// Fee policies that will become effective at a future timestamp.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct UpcomingFeePolicies {
    /// Policies that will replace the current ones at the effective timestamp.
    #[serde(default)]
    pub policies: Vec<FeePolicy>,

    /// Timestamp from which the `policies` will become active.
    pub effective_from_timestamp: Option<DateTime<Utc>>,
}

fn one_or_many<'de, D>(deserializer: D) -> Result<Vec<UpcomingFeePolicies>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OneOrMany;

    impl<'de> Visitor<'de> for OneOrMany {
        type Value = Vec<UpcomingFeePolicies>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a table or an array of tables")
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            UpcomingFeePolicies::deserialize(MapAccessDeserializer::new(map))
                .map(|upcoming| vec![upcoming])
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            Vec::deserialize(SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(OneOrMany)
}

/// Method used to calculate the protocol fee for an order.
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

/// Which order class a fee policy targets.
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
        assert!(config.policies.is_empty());
        assert_eq!(config.max_partner_fee.get(), 0.01);
        assert!(config.upcoming_policies.is_empty());
    }

    #[test]
    fn deserialize_fee_policies_config_full() {
        let toml = r#"
        max-partner-fee = 0.005

        [[policies]]
        kind.surplus = { factor = 0.5, max-volume-factor = 0.9 }
        order-class = "limit"

        [[policies]]
        kind.volume = { factor = 0.1 }
        order-class = "any"

        [upcoming-policies]
        effective-from-timestamp = "2025-06-01T00:00:00Z"

        [[upcoming-policies.policies]]
        kind.volume = { factor = 0.2 }
        order-class = "any"
        "#;
        let config: FeePoliciesConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.policies.len(), 2);
        assert_eq!(config.max_partner_fee.get(), 0.005);
        assert_eq!(config.upcoming_policies.len(), 1);
        assert_eq!(config.upcoming_policies[0].policies.len(), 1);
        assert!(
            config.upcoming_policies[0]
                .effective_from_timestamp
                .is_some()
        );
    }

    #[test]
    fn deserialize_upcoming_fee_policies_list() {
        let toml = r#"
        [[upcoming-policies]]
        effective-from-timestamp = "2025-06-01T00:00:00Z"

        [[upcoming-policies.policies]]
        kind.volume = { factor = 0.2 }
        order-class = "any"

        [[upcoming-policies]]
        effective-from-timestamp = "2025-07-01T00:00:00Z"

        [[upcoming-policies.policies]]
        kind.volume = { factor = 0.3 }
        order-class = "any"
        "#;
        let config: FeePoliciesConfig = toml::from_str(toml).unwrap();
        let timestamps: Vec<_> = config
            .upcoming_policies
            .iter()
            .map(|upcoming| upcoming.effective_from_timestamp.unwrap().to_rfc3339())
            .collect();
        assert_eq!(
            timestamps,
            ["2025-06-01T00:00:00+00:00", "2025-07-01T00:00:00+00:00"]
        );
    }

    #[test]
    fn upcoming_fee_policies_list_round_trips() {
        let config: FeePoliciesConfig = toml::from_str(
            r#"
            [[upcoming-policies]]
            effective-from-timestamp = "2025-06-01T00:00:00Z"

            [[upcoming-policies.policies]]
            kind.volume = { factor = 0.2 }
            order-class = "any"
            "#,
        )
        .unwrap();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: FeePoliciesConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.upcoming_policies.len(), 1);
        assert_eq!(deserialized.upcoming_policies[0].policies.len(), 1);
    }

    #[test]
    fn deserialize_upcoming_fee_policies_reports_the_bad_field() {
        let toml = r#"
        [upcoming-policies]
        effective-from-timestamp = "2025-06-01T00:00:00Z"

        [[upcoming-policies.policies]]
        kind.volume = { factor = 1.5 }
        order-class = "any"
        "#;
        let error = toml::from_str::<FeePoliciesConfig>(toml).unwrap_err();
        assert!(!error.to_string().contains("a table or an array of tables"));
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
