use {
    serde::{Deserialize, Serialize},
    shared::price_estimation::NativePriceEstimators,
    std::{num::NonZeroUsize, time::Duration},
};

const fn default_native_price_estimation_results_required() -> NonZeroUsize {
    NonZeroUsize::new(2).expect("value should not be zero")
}

const fn default_native_price_cache_refresh() -> Duration {
    Duration::from_secs(1)
}

const fn default_native_price_prefetch_time() -> Duration {
    Duration::from_secs(80)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NativePriceConfig {
    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token. Estimators with the same name need to also be specified as
    /// built-in, legacy or external price estimators (lookup happens in this
    /// order in case of name collisions)
    pub estimators: NativePriceEstimators,

    /// Estimators for the API endpoint. Falls back to
    /// `--native-price-estimators` if unset.
    pub api_estimators: Option<NativePriceEstimators>,

    /// How many successful price estimates for each order will cause a native
    /// price estimation to return its result early. It's possible to pass
    /// values greater than the total number of enabled estimators but that
    /// will not have any further effect.
    #[serde(default = "default_native_price_estimation_results_required")]
    pub results_required: NonZeroUsize,

    /// How often the native price estimator should check for prices that need
    /// to be udpated.
    #[serde(
        with = "humantime_serde",
        default = "default_native_price_cache_refresh"
    )]
    pub cache_refresh_interval: Duration,

    /// How long before expiry the native price cache should try to update the
    /// price in the background. This value has to be smaller than
    /// `--native-price-cache-max-age`.
    #[serde(
        with = "humantime_serde",
        default = "default_native_price_prefetch_time"
    )]
    pub prefetch_time: Duration,
}

impl Default for NativePriceConfig {
    fn default() -> Self {
        Self {
            estimators: Default::default(),
            api_estimators: Default::default(),
            results_required: default_native_price_estimation_results_required(),
            cache_refresh_interval: default_native_price_cache_refresh(),
            prefetch_time: default_native_price_prefetch_time(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let toml = r#"
        estimators = []
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert!(config.estimators.as_slice().is_empty());
        assert!(config.api_estimators.is_none());
        assert_eq!(config.results_required.get(), 2);
        assert_eq!(config.cache_refresh_interval, Duration::from_secs(1));
        assert_eq!(config.prefetch_time, Duration::from_secs(80));
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        estimators = [[{type = "CoinGecko"}]]
        api-estimators = [[{type = "OneInchSpotPriceApi"}]]
        results-required = 3
        cache-refresh-interval = "30s"
        prefetch-time = "2m"
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.estimators.as_slice().len(), 1);
        assert!(config.api_estimators.is_some());
        assert_eq!(config.results_required.get(), 3);
        assert_eq!(config.cache_refresh_interval, Duration::from_secs(30));
        assert_eq!(config.prefetch_time, Duration::from_secs(120));
    }

    #[test]
    fn missing_estimators_fails() {
        let toml = "";
        assert!(toml::from_str::<NativePriceConfig>(toml).is_err());
    }

    #[test]
    fn default_impl() {
        let config = NativePriceConfig::default();
        assert!(config.estimators.as_slice().is_empty());
        assert!(config.api_estimators.is_none());
        assert_eq!(config.results_required.get(), 2);
        assert_eq!(config.cache_refresh_interval, Duration::from_secs(1));
        assert_eq!(config.prefetch_time, Duration::from_secs(80));
    }
}
