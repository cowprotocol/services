use {
    serde::{Deserialize, Serialize},
    shared::price_estimation::{
        NativePriceEstimators,
        config::native_price::NativePriceConfig as SharedNativePriceConfig,
    },
    std::num::NonZeroUsize,
};

const fn default_results_required() -> NonZeroUsize {
    NonZeroUsize::new(2).expect("results required should be greater than 0")
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NativePriceConfig {
    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token.
    pub estimators: NativePriceEstimators,

    /// Fallback native price estimators to use when all primary estimators
    /// are down.
    pub fallback_estimators: Option<NativePriceEstimators>,

    /// How many successful price estimates for each order will cause a fast
    /// or native price estimation to return its result early.
    /// The bigger the value the more the fast price estimation performs like
    /// the optimal price estimation.
    /// It's possible to pass values greater than the total number of enabled
    /// estimators but that will not have any further effect.
    #[serde(default = "default_results_required")]
    pub results_required: NonZeroUsize,

    #[serde(flatten)]
    pub shared: SharedNativePriceConfig,
}

impl Default for NativePriceConfig {
    fn default() -> Self {
        Self {
            estimators: Default::default(),
            fallback_estimators: Default::default(),
            results_required: default_results_required(),
            shared: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let config: NativePriceConfig = toml::from_str("estimators = []").unwrap();
        assert_eq!(config.results_required.get(), 2);
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
        assert_eq!(config.results_required.get(), 3);
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
        let config = NativePriceConfig {
            estimators: NativePriceEstimators::new(vec![vec![
                shared::price_estimation::NativePriceEstimator::CoinGecko,
            ]]),
            fallback_estimators: Some(NativePriceEstimators::new(vec![vec![
                shared::price_estimation::NativePriceEstimator::OneInchSpotPriceApi,
            ]])),
            results_required: NonZeroUsize::new(5).unwrap(),
            ..Default::default()
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: NativePriceConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(config.results_required, deserialized.results_required,);
    }
}
