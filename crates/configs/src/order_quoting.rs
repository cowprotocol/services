use {serde::Deserialize, std::time::Duration, url::Url};

const fn default_eip1271_onchain_quote_validity() -> Duration {
    Duration::from_mins(10)
}
const fn default_presign_onchain_quote_validity() -> Duration {
    Duration::from_mins(10)
}
const fn default_standard_offchain_quote_validity() -> Duration {
    Duration::from_mins(1)
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case")]
pub struct ExternalSolver {
    pub name: String,
    pub url: Url,
}

// The following arguments are used to configure the order creation process.
#[derive(Debug, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case")]
pub struct OrderQuoting {
    /// A list of external drivers used for price estimation.
    pub price_estimation_drivers: Vec<ExternalSolver>,

    /// The time period an EIP1271-quote request is valid.
    #[serde(
        with = "humantime_serde",
        default = "default_eip1271_onchain_quote_validity"
    )]
    pub eip1271_onchain_quote_validity: Duration,

    /// The time period an PRESIGN-quote request is valid.
    #[serde(
        with = "humantime_serde",
        default = "default_presign_onchain_quote_validity"
    )]
    pub presign_onchain_quote_validity: Duration,

    /// The time period a regular offchain-quote request (ethsign/eip712) is
    /// valid.
    #[serde(
        with = "humantime_serde",
        default = "default_standard_offchain_quote_validity"
    )]
    pub standard_offchain_quote_validity: Duration,
}

#[cfg(any(test, feature = "test-util"))]
impl ExternalSolver {
    pub fn new(name: &str, url: &str) -> Self {
        Self {
            name: name.to_string(),
            url: url.parse().unwrap(),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl OrderQuoting {
    pub fn test_with_drivers(drivers: Vec<ExternalSolver>) -> Self {
        Self {
            price_estimation_drivers: drivers,
            ..crate::test_util::TestDefault::test_default()
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for OrderQuoting {
    fn test_default() -> Self {
        Self {
            price_estimation_drivers: vec![],
            eip1271_onchain_quote_validity: default_eip1271_onchain_quote_validity(),
            presign_onchain_quote_validity: default_presign_onchain_quote_validity(),
            standard_offchain_quote_validity: default_standard_offchain_quote_validity(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let toml = r#"
        price-estimation-drivers = []
        "#;
        let config: OrderQuoting = toml::from_str(toml).unwrap();
        assert!(config.price_estimation_drivers.is_empty());
        assert_eq!(
            config.eip1271_onchain_quote_validity,
            Duration::from_mins(10)
        );
        assert_eq!(
            config.presign_onchain_quote_validity,
            Duration::from_mins(10)
        );
        assert_eq!(
            config.standard_offchain_quote_validity,
            Duration::from_mins(1)
        );
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        eip1271-onchain-quote-validity = "5m"
        presign-onchain-quote-validity = "20m"
        standard-offchain-quote-validity = "30s"

        [[price-estimation-drivers]]
        name = "test-solver"
        url = "http://localhost:8080"
        "#;
        let config: OrderQuoting = toml::from_str(toml).unwrap();
        assert_eq!(config.price_estimation_drivers.len(), 1);
        assert_eq!(config.price_estimation_drivers[0].name, "test-solver");
        assert_eq!(
            config.price_estimation_drivers[0].url.as_str(),
            "http://localhost:8080/"
        );
        assert_eq!(
            config.eip1271_onchain_quote_validity,
            Duration::from_mins(5)
        );
        assert_eq!(
            config.presign_onchain_quote_validity,
            Duration::from_mins(20)
        );
        assert_eq!(
            config.standard_offchain_quote_validity,
            Duration::from_secs(30)
        );
    }

    #[test]
    fn deserialize_multiple_drivers() {
        let toml = r#"
        [[price-estimation-drivers]]
        name = "solver-a"
        url = "http://solver-a:8080"

        [[price-estimation-drivers]]
        name = "solver-b"
        url = "http://solver-b:9090"
        "#;
        let config: OrderQuoting = toml::from_str(toml).unwrap();
        assert_eq!(config.price_estimation_drivers.len(), 2);
        assert_eq!(config.price_estimation_drivers[0].name, "solver-a");
        assert_eq!(config.price_estimation_drivers[1].name, "solver-b");
    }

    #[test]
    fn deserialize_missing_required_field() {
        let toml = "";
        let result = toml::from_str::<OrderQuoting>(toml);
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_serialization() {
        let config = OrderQuoting {
            price_estimation_drivers: vec![ExternalSolver {
                name: "test".to_string(),
                url: "http://localhost:1234".parse().unwrap(),
            }],
            eip1271_onchain_quote_validity: Duration::from_secs(300),
            presign_onchain_quote_validity: Duration::from_secs(600),
            standard_offchain_quote_validity: Duration::from_secs(60),
        };
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: OrderQuoting = toml::from_str(&serialized).unwrap();
        assert_eq!(
            config.eip1271_onchain_quote_validity,
            deserialized.eip1271_onchain_quote_validity
        );
        assert_eq!(
            config.presign_onchain_quote_validity,
            deserialized.presign_onchain_quote_validity
        );
        assert_eq!(
            config.standard_offchain_quote_validity,
            deserialized.standard_offchain_quote_validity
        );
        assert_eq!(
            config.price_estimation_drivers.len(),
            deserialized.price_estimation_drivers.len()
        );
    }
}
