use {
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

fn default_eviction_time() -> Duration {
    Duration::from_secs(20)
}

fn default_refresh_delay() -> Duration {
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

    /// How long the background refresh waits after a new block, so it does not
    /// compete with the RPC burst at the block edge. Blocks arriving inside the
    /// window share one refresh: at the 1s default a chain with 400ms blocks
    /// refreshes every ~3rd block rather than every block.
    #[serde(with = "humantime_serde", default = "default_refresh_delay")]
    pub refresh_delay: Duration,
}

impl Default for BalanceCacheConfig {
    fn default() -> Self {
        Self {
            eviction_time: default_eviction_time(),
            refresh_delay: default_refresh_delay(),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for BalanceCacheConfig {
    fn test_default() -> Self {
        // Refresh on every block without delay so tests stay deterministic.
        Self {
            eviction_time: default_eviction_time(),
            refresh_delay: Duration::ZERO,
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
        assert_eq!(config.refresh_delay, Duration::from_secs(1));
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        eviction-time = "30s"
        refresh-delay = "500ms"
        "#;
        let config: BalanceCacheConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.eviction_time, Duration::from_secs(30));
        assert_eq!(config.refresh_delay, Duration::from_millis(500));
    }
}
