use {
    serde::{Deserialize, Serialize},
    shared::price_estimation::NativePriceEstimators,
    std::time::Duration,
};

const fn default_native_price_cache_refresh() -> Duration {
    Duration::from_secs(1)
}

const fn default_native_price_prefetch_time() -> Duration {
    Duration::from_secs(80)
}

// Does not implement Default because `estimators` *cannot* be empty,
// as such, we cannot provide a proper default value for this structure.
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

    #[serde(flatten)]
    pub shared: shared::price_estimation::config::native_price::NativePriceConfig,
}

#[cfg(any(test, feature = "test-util"))]
impl NativePriceConfig {
    /// Test configuration for [`NativePriceConfig`], must always be able to do
    /// a serialization/deserialization roundtrip, as otherwise it may not
    /// be loadable in end-to-end tests.
    pub fn test_default() -> Self {
        Self {
            estimators: NativePriceEstimators::test_default(),
            api_estimators: Default::default(),
            cache_refresh_interval: default_native_price_cache_refresh(),
            prefetch_time: Duration::from_millis(500),
            shared: shared::price_estimation::config::native_price::NativePriceConfig {
                cache: shared::price_estimation::config::native_price::CacheConfig {
                    max_age: Duration::from_secs(2),
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_full() {
        let toml = r#"
        estimators = [[{type = "CoinGecko"}]]
        api-estimators = [[{type = "OneInchSpotPriceApi"}]]
        cache-refresh-interval = "30s"
        prefetch-time = "2m"
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.estimators.as_slice().len(), 1);
        assert!(config.api_estimators.is_some());
        assert_eq!(config.cache_refresh_interval, Duration::from_secs(30));
        assert_eq!(config.prefetch_time, Duration::from_secs(120));
    }

    #[test]
    fn missing_estimators_fails() {
        let toml = "";
        assert!(toml::from_str::<NativePriceConfig>(toml).is_err());
    }

    // This test keeps the sanity of `test_default` upon which other tests rely!
    #[test]
    fn test_default_roundtrip() {
        let config = NativePriceConfig::test_default();
        let serialized = toml::to_string(&config).unwrap();
        let _: NativePriceConfig = toml::from_str(&serialized).unwrap();
    }
}
