use {
    alloy::primitives::Address,
    serde::{Deserialize, Serialize},
    std::num::NonZeroUsize,
};

fn default_max_cache_size() -> NonZeroUsize {
    NonZeroUsize::new(10000).unwrap()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BannedUsersConfig {
    /// List of account addresses to be denied from order creation.
    #[serde(default)]
    pub addresses: Vec<Address>,

    /// Maximum number of entries to keep in the banned users cache.
    #[serde(default = "default_max_cache_size")]
    pub max_cache_size: NonZeroUsize,
}

impl Default for BannedUsersConfig {
    fn default() -> Self {
        Self {
            addresses: Vec::new(),
            max_cache_size: default_max_cache_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::address};

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: BannedUsersConfig = toml::from_str(toml).unwrap();
        assert!(config.addresses.is_empty());
        assert_eq!(config.max_cache_size.get(), 10000);
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        addresses = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        max-cache-size = 5000
        "#;
        let config: BannedUsersConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.addresses.len(), 1);
        assert_eq!(
            config.addresses[0],
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
        );
        assert_eq!(config.max_cache_size.get(), 5000);
    }
}
