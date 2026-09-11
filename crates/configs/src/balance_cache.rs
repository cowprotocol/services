use {
    anyhow::ensure,
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

fn default_eviction_time() -> Duration {
    Duration::from_secs(20)
}

fn default_refresh_cooldown() -> Duration {
    Duration::from_secs(1)
}

/// Settings for the on-chain balance cache used by services that fetch trader
/// balances during auction preparation or quoting.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BalanceCacheConfig {
    /// Cached balances that have not been queried within this duration get
    /// evicted on the next background refresh. Below the auction cadence the
    /// cache degrades into a pass-through.
    #[serde(with = "humantime_serde", default = "default_eviction_time")]
    pub eviction_time: Duration,

    /// Minimum time between two background refreshes. Blocks arriving inside
    /// the window share one refresh: at the 1s default a chain with 400ms
    /// blocks refreshes every ~3rd block rather than every block.
    #[serde(with = "humantime_serde", default = "default_refresh_cooldown")]
    pub refresh_cooldown: Duration,
}

impl BalanceCacheConfig {
    /// Cross-field invariants that cannot be expressed in the serde schema.
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.eviction_time > self.refresh_cooldown,
            "`eviction-time` ({:?}) must be longer than `refresh-cooldown` ({:?}), otherwise \
             every entry is evicted before it can be refreshed",
            self.eviction_time,
            self.refresh_cooldown,
        );
        Ok(())
    }
}

impl Default for BalanceCacheConfig {
    fn default() -> Self {
        Self {
            eviction_time: default_eviction_time(),
            refresh_cooldown: default_refresh_cooldown(),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for BalanceCacheConfig {
    fn test_default() -> Self {
        // Refresh on every block without a cooldown so tests stay
        // deterministic.
        Self {
            eviction_time: default_eviction_time(),
            refresh_cooldown: Duration::ZERO,
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
        assert_eq!(config.refresh_cooldown, Duration::from_secs(1));
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        eviction-time = "30s"
        refresh-cooldown = "500ms"
        "#;
        let config: BalanceCacheConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.eviction_time, Duration::from_secs(30));
        assert_eq!(config.refresh_cooldown, Duration::from_millis(500));
    }

    #[test]
    fn validate_rejects_eviction_time_below_refresh_cooldown() {
        assert!(BalanceCacheConfig::default().validate().is_ok());

        let config = BalanceCacheConfig {
            eviction_time: Duration::from_secs(1),
            refresh_cooldown: Duration::from_secs(1),
        };
        assert!(config.validate().is_err());

        let config = BalanceCacheConfig {
            eviction_time: Duration::from_millis(500),
            refresh_cooldown: Duration::from_secs(1),
        };
        assert!(config.validate().is_err());
    }
}
