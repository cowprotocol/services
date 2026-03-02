use {
    serde::{Deserialize, Serialize},
    std::num::{NonZeroU64, NonZeroUsize},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BalancesCacheConfig {
    /// For how many blocks a particular balances must not have been
    /// requested before getting evicted from the cache.
    #[serde(default = "default_max_request_age")]
    pub max_request_age: NonZeroU64,

    /// How many balances may be fetched at most in parallel by the
    /// background task.
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

fn default_max_request_age() -> NonZeroU64 {
    NonZeroU64::new(5).unwrap()
}

fn default_max_concurrent_updates() -> NonZeroUsize {
    NonZeroUsize::new(100).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: BalancesCacheConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_request_age.get(), 5);
        assert_eq!(config.max_concurrent_updates.get(), 10000);
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        max-request-age = 1
        max-concurrent-updates = 1
        "#;
        let config: BalancesCacheConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_request_age.get(), 1);
        assert_eq!(config.max_concurrent_updates.get(), 1);
    }
}
