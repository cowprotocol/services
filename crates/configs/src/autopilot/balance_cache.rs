use {
    serde::Deserialize,
    std::num::{NonZeroU64, NonZeroUsize},
};

fn default_max_request_age() -> NonZeroU64 {
    NonZeroU64::new(5).unwrap()
}

fn default_max_concurrent_updates() -> NonZeroUsize {
    NonZeroUsize::new(100).unwrap()
}

/// Configuration for the user balances cache and its background refresh task.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BalancesCacheConfig {
    /// Number of blocks a balance entry may go without being requested before
    /// it is evicted from the cache.
    #[serde(default = "default_max_request_age")]
    pub max_request_age: NonZeroU64,

    /// Maximum number of balance fetches that the background refresh task may
    /// run concurrently.
    #[serde(default = "default_max_concurrent_updates")]
    pub max_concurrent_updates: NonZeroUsize,
}

impl Default for BalancesCacheConfig {
    fn default() -> Self {
        Self {
            max_request_age: default_max_request_age(),
            max_concurrent_updates: default_max_concurrent_updates(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let config: BalancesCacheConfig = toml::from_str("").unwrap();
        assert_eq!(config.max_request_age.get(), 5);
        assert_eq!(config.max_concurrent_updates.get(), 100);
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        max-request-age = 10
        max-concurrent-updates = 50
        "#;
        let config: BalancesCacheConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_request_age.get(), 10);
        assert_eq!(config.max_concurrent_updates.get(), 50);
    }
}
