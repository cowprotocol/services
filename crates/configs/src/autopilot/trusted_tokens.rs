use {
    alloy::primitives::Address,
    serde::{Deserialize, Serialize},
    std::time::Duration,
    url::Url,
};

fn default_update_interval() -> Duration {
    Duration::from_secs(3600) // 1h
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TrustedTokensConfig {
    /// The URL of a list of tokens our settlement contract is willing to
    /// internalize.
    pub url: Option<Url>,

    /// Hardcoded list of trusted tokens to use in addition to `url`.
    #[serde(default)]
    pub tokens: Vec<Address>,

    /// Time interval after which the trusted tokens list needs to be updated.
    #[serde(with = "humantime_serde", default = "default_update_interval")]
    pub update_interval: Duration,
}

impl Default for TrustedTokensConfig {
    fn default() -> Self {
        Self {
            url: None,
            tokens: Vec::new(),
            update_interval: default_update_interval(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: TrustedTokensConfig = toml::from_str(toml).unwrap();
        assert!(config.url.is_none());
        assert!(config.tokens.is_empty());
        assert_eq!(config.update_interval, Duration::from_secs(3600));
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        url = "https://example.com/tokens.json"
        tokens = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        update-interval = "30m"
        "#;
        let config: TrustedTokensConfig = toml::from_str(toml).unwrap();
        assert!(config.url.is_some());
        assert_eq!(config.tokens.len(), 1);
        assert_eq!(config.update_interval, Duration::from_secs(1800));
    }
}
