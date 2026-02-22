use {
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

const fn default_min_order_validity_period() -> Duration {
    Duration::from_secs(60) // 1m
}

const fn default_max_order_validity_period() -> Duration {
    Duration::from_secs(10800) // 3h
}

const fn default_max_limit_order_validity_period() -> Duration {
    Duration::from_secs(31_536_000) // 1y
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct OrderValidationConfig {
    /// The minimum amount of time an order has to be valid for.
    #[serde(
        with = "humantime_serde",
        default = "default_min_order_validity_period"
    )]
    pub min_order_validity_period: Duration,

    /// The maximum amount of time an order can be valid for.
    /// This restriction does not apply to liquidity owner orders or presign
    /// orders.
    #[serde(
        with = "humantime_serde",
        default = "default_max_order_validity_period"
    )]
    pub max_order_validity_period: Duration,

    /// The maximum amount of time a limit order can be valid for.
    #[serde(
        with = "humantime_serde",
        default = "default_max_limit_order_validity_period"
    )]
    pub max_limit_order_validity_period: Duration,
}

impl Default for OrderValidationConfig {
    fn default() -> Self {
        Self {
            min_order_validity_period: default_min_order_validity_period(),
            max_order_validity_period: default_max_order_validity_period(),
            max_limit_order_validity_period: default_max_limit_order_validity_period(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let config: OrderValidationConfig = toml::from_str("").unwrap();
        assert_eq!(config.min_order_validity_period, Duration::from_secs(60));
        assert_eq!(config.max_order_validity_period, Duration::from_secs(10800));
        assert_eq!(
            config.max_limit_order_validity_period,
            Duration::from_secs(31_536_000)
        );
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        min-order-validity-period = "2m"
        max-order-validity-period = "6h"
        max-limit-order-validity-period = "30d"
        "#;
        let config: OrderValidationConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.min_order_validity_period, Duration::from_secs(120));
        assert_eq!(config.max_order_validity_period, Duration::from_secs(21600));
        assert_eq!(
            config.max_limit_order_validity_period,
            Duration::from_secs(2_592_000)
        );
    }

    #[test]
    fn roundtrip_serialization() {
        let config = OrderValidationConfig {
            min_order_validity_period: Duration::from_secs(120),
            max_order_validity_period: Duration::from_secs(7200),
            max_limit_order_validity_period: Duration::from_secs(86400),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: OrderValidationConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(
            config.min_order_validity_period,
            deserialized.min_order_validity_period
        );
        assert_eq!(
            config.max_order_validity_period,
            deserialized.max_order_validity_period
        );
        assert_eq!(
            config.max_limit_order_validity_period,
            deserialized.max_limit_order_validity_period
        );
    }
}
