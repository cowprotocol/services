use {
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

fn default_cleanup_interval() -> Duration {
    Duration::from_secs(86400) // 1d
}

fn default_cleanup_threshold() -> Duration {
    Duration::from_secs(2592000) // 30d
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct OrderEventsCleanupConfig {
    /// Time interval between each cleanup operation of the `order_events`
    /// database table.
    #[serde(with = "humantime_serde", default = "default_cleanup_interval")]
    pub cleanup_interval: Duration,

    /// Age threshold for order events to be eligible for cleanup in the
    /// `order_events` database table.
    #[serde(with = "humantime_serde", default = "default_cleanup_threshold")]
    pub cleanup_threshold: Duration,
}

impl Default for OrderEventsCleanupConfig {
    fn default() -> Self {
        Self {
            cleanup_interval: default_cleanup_interval(),
            cleanup_threshold: default_cleanup_threshold(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: OrderEventsCleanupConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.cleanup_interval, Duration::from_secs(86400));
        assert_eq!(config.cleanup_threshold, Duration::from_secs(2592000));
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        cleanup-interval = "12h"
        cleanup-threshold = "7d"
        "#;
        let config: OrderEventsCleanupConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.cleanup_interval, Duration::from_secs(43200));
        assert_eq!(config.cleanup_threshold, Duration::from_secs(604800));
    }
}
