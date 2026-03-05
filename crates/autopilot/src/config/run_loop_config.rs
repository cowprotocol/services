use {serde::Deserialize, std::time::Duration};

fn default_max_delay() -> Duration {
    Duration::from_secs(2)
}

/// Configuration for the autopilot run loop timing.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RunLoopConfig {
    /// If a new run loop iteration would start more than this duration after
    /// the latest block was noticed, wait for the next block before continuing.
    #[serde(with = "humantime_serde", default = "default_max_delay")]
    pub max_delay: Duration,

    /// Maximum timeout for fetching native prices in the run loop.
    /// If 0, native prices are fetched from cache.
    #[serde(with = "humantime_serde", default)]
    pub native_price_timeout: Duration,
}

impl Default for RunLoopConfig {
    fn default() -> Self {
        Self {
            max_delay: default_max_delay(),
            native_price_timeout: Duration::ZERO,
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl configs::test_util::TestDefault for RunLoopConfig {
    fn test_default() -> Self {
        Self {
            max_delay: Duration::from_millis(100),
            native_price_timeout: Duration::from_millis(500),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let config: RunLoopConfig = toml::from_str("").unwrap();
        assert_eq!(config.max_delay, Duration::from_secs(2));
        assert_eq!(config.native_price_timeout, Duration::ZERO);
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        max-delay = "5s"
        native-price-timeout = "500ms"
        "#;
        let config: RunLoopConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_delay, Duration::from_secs(5));
        assert_eq!(config.native_price_timeout, Duration::from_millis(500));
    }
}
