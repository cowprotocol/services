use {
    alloy::primitives::Address,
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

const fn default_cache_max_age() -> Duration {
    Duration::from_mins(10)
}

const fn default_cache_concurrent_requests() -> usize {
    1
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct NativePriceConfig {
    /// List of mappings of native price tokens substitutions with approximated:
    /// - the first is a token address for which we get the native token price
    /// - the second is a token address used for the price approximation
    #[serde(default)]
    pub approximation_tokens: Vec<(Address, Address)>,

    /// Configuration for the native price caching mechanism.
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct CacheConfig {
    /// How long cached native prices stay valid.
    #[serde(default = "default_cache_max_age", with = "humantime_serde")]
    pub max_age: Duration,

    /// How many price estimation requests can be executed concurrently in the
    /// maintenance task.
    #[serde(default = "default_cache_concurrent_requests")]
    pub concurrent_requests: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_age: default_cache_max_age(),
            concurrent_requests: default_cache_concurrent_requests(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_full() {
        let toml = r#"
            approximation-tokens = [
                ["0x0000000000000000000000000000000000000001", "0x0000000000000000000000000000000000000002"],
            ]

            [cache]
            max-age = "5m"
            concurrent-requests = 4
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.approximation_tokens.len(), 1);
        assert_eq!(config.cache.max_age, Duration::from_secs(300));
        assert_eq!(config.cache.concurrent_requests, 4);
    }

    #[test]
    fn cache_defaults() {
        let toml = r#"
            approximation-tokens = []
            [cache]
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.cache.max_age, Duration::from_mins(10));
        assert_eq!(config.cache.concurrent_requests, 1);
    }

    #[test]
    fn multiple_approximation_tokens() {
        let toml = r#"
            approximation-tokens = [
                ["0x0000000000000000000000000000000000000001", "0x0000000000000000000000000000000000000002"],
                ["0x0000000000000000000000000000000000000003", "0x0000000000000000000000000000000000000004"],
            ]
            [cache]
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.approximation_tokens.len(), 2);
        assert_eq!(
            config.approximation_tokens[0].0,
            Address::from_slice(&[0; 19].into_iter().chain([1]).collect::<Vec<_>>()),
        );
    }

    #[test]
    fn roundtrip_serialization() {
        let config = NativePriceConfig {
            approximation_tokens: vec![(Address::repeat_byte(1), Address::repeat_byte(2))],
            cache: CacheConfig {
                max_age: Duration::from_secs(120),
                concurrent_requests: 8,
            },
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: NativePriceConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(
            config.approximation_tokens,
            deserialized.approximation_tokens,
        );
        assert_eq!(config.cache.max_age, deserialized.cache.max_age);
        assert_eq!(
            config.cache.concurrent_requests,
            deserialized.cache.concurrent_requests,
        );
    }
}
