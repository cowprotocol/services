use {
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

fn default_eviction_time() -> Duration {
    Duration::from_secs(20)
}

fn default_min_update_interval() -> Duration {
    Duration::from_secs(1)
}

/// Settings for the on-chain balance cache used by services that fetch trader
/// balances during auction preparation or quoting.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BalanceCacheConfig {
    /// Cached balances that have not been queried within this duration get
    /// evicted on the next background refresh. Should be longer than a typical
    /// auction so that the entries survive across auctions.
    #[serde(with = "humantime_serde", default = "default_eviction_time")]
    pub eviction_time: Duration,

    /// Minimum time between two background refreshes of the cache. New blocks
    /// arriving inside this window are coalesced into a single refresh so that
    /// fast chains don't burn CPU refetching balances every block.
    #[serde(with = "humantime_serde", default = "default_min_update_interval")]
    pub min_update_interval: Duration,
}

impl Default for BalanceCacheConfig {
    fn default() -> Self {
        Self {
            eviction_time: default_eviction_time(),
            min_update_interval: default_min_update_interval(),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for BalanceCacheConfig {
    fn test_default() -> Self {
        // Disable throttling in tests so the cache reacts to every block.
        Self {
            eviction_time: default_eviction_time(),
            min_update_interval: Duration::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: BalanceCacheConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.eviction_time, Duration::from_secs(20));
        assert_eq!(config.min_update_interval, Duration::from_secs(1));
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        eviction-time = "30s"
        min-update-interval = "500ms"
        "#;
        let config: BalanceCacheConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.eviction_time, Duration::from_secs(30));
        assert_eq!(config.min_update_interval, Duration::from_millis(500));
    }
}
