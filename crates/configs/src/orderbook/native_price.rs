use {
    crate::price_estimation::{
        NativePriceConfig as SharedNativePriceConfig,
        NativePriceEstimators,
    },
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NativePriceConfig {
    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token.
    pub estimators: NativePriceEstimators,

    /// Fallback native price estimators to use when all primary estimators
    /// are down.
    pub fallback_estimators: Option<NativePriceEstimators>,

    #[serde(flatten)]
    pub shared: SharedNativePriceConfig,
}

#[cfg(any(test, feature = "test-util"))]
impl NativePriceConfig {
    /// The orderbook forwards native price requests to the autopilot.
    pub fn test_default() -> Self {
        use crate::price_estimation::NativePriceEstimator;
        Self {
            estimators: NativePriceEstimators::new(vec![vec![NativePriceEstimator::forwarder(
                "http://localhost:12088".parse().unwrap(),
            )]]),
            fallback_estimators: Default::default(),
            shared: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let config: NativePriceConfig =
            toml::from_str(r#"estimators = [[{type = "CoinGecko"}]]"#).unwrap();
        assert_eq!(config.estimators.as_slice().len(), 1);
        assert!(config.fallback_estimators.is_none());
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
            estimators = [[{ type = "CoinGecko" }, { type = "OneInchSpotPriceApi" }]]
            fallback-estimators = [[{ type = "CoinGecko" }]]
            results-required = 3
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert!(config.fallback_estimators.is_some());
    }

    #[test]
    fn deserialize_driver_estimator() {
        let toml = r#"
            estimators = [[{ type = "Driver", name = "test", url = "http://localhost:8080" }]]
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert!(!config.estimators.as_slice().is_empty());
    }

    #[test]
    fn roundtrip_serialization() {
        use crate::price_estimation::NativePriceEstimator;

        let config = NativePriceConfig {
            estimators: NativePriceEstimators::new(vec![vec![NativePriceEstimator::CoinGecko]]),
            fallback_estimators: Some(NativePriceEstimators::new(vec![vec![
                NativePriceEstimator::OneInchSpotPriceApi,
            ]])),
            shared: Default::default(),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: NativePriceConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.estimators.as_slice(),
            config.estimators.as_slice(),
        );
        assert_eq!(
            deserialized
                .fallback_estimators
                .as_ref()
                .map(|e| e.as_slice()),
            config.fallback_estimators.as_ref().map(|e| e.as_slice()),
        );
    }
}
