use {
    alloy::primitives::Address,
    serde::{Deserialize, Serialize},
    url::Url,
};

/// Configuration for indexing CoW AMMs.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CowAmmGroupConfig {
    /// List of CoW AMM factory/helper/start-block configurations.
    #[serde(default)]
    pub contracts: Vec<CowAmmConfig>,

    /// Archive node URL used to index CoW AMMs.
    #[serde(default)]
    pub archive_node_url: Option<Url>,
}

/// A single CoW AMM factory configuration.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CowAmmConfig {
    /// Contract address emitting CoW AMM deployment events.
    pub factory: Address,

    /// Contract address to interface with pools deployed by the factory.
    pub helper: Address,

    /// Block at which indexing should start (1 block before factory
    /// deployment).
    pub index_start: u64,
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::address};

    #[test]
    fn deserialize_defaults() {
        let config: CowAmmGroupConfig = toml::from_str("").unwrap();
        assert!(config.contracts.is_empty());
        assert!(config.archive_node_url.is_none());
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        archive-node-url = "http://archive.example.com"

        [[contracts]]
        factory = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        helper = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
        index-start = 12345678

        [[contracts]]
        factory = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        helper = "0x6B175474E89094C44Da98b954EedeAC495271d0F"
        index-start = 99999999
        "#;
        let config: CowAmmGroupConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.contracts.len(), 2);
        assert_eq!(
            config.contracts[0].factory,
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
        );
        assert_eq!(config.contracts[0].index_start, 12345678);
        assert_eq!(
            config.archive_node_url.unwrap().as_str(),
            "http://archive.example.com/"
        );
    }
}
