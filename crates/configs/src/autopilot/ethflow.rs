use {alloy::primitives::Address, serde::Deserialize};

/// Configuration for eth-flow order indexing and processing.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct EthflowConfig {
    /// Address of the ethflow contracts. If empty, eth-flow orders are
    /// disabled. Multiple contracts supported for migration transition
    /// periods.
    #[serde(default)]
    pub contracts: Vec<Address>,

    /// Timestamp at which we should start indexing eth-flow contract events.
    /// Ignored if events already exist in the database for a later date.
    pub indexing_start: Option<u64>,

    /// Skip syncing past events (useful for local deployments).
    #[serde(default)]
    pub skip_event_sync: bool,
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for EthflowConfig {
    fn test_default() -> Self {
        Self {
            skip_event_sync: true,
            // In E2E tests the contracts are added later
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let config: EthflowConfig = toml::from_str("").unwrap();
        assert!(config.contracts.is_empty());
        assert!(config.indexing_start.is_none());
        assert!(!config.skip_event_sync);
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        contracts = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        indexing-start = 12345678
        skip-event-sync = true
        "#;
        let config: EthflowConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.contracts.len(), 1);
        assert_eq!(config.indexing_start, Some(12345678));
        assert!(config.skip_event_sync);
    }
}
