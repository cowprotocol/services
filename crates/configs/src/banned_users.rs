use {
    crate::deserialize_env::{deserialize_optional_string_from_env, deserialize_string_from_env},
    alloy::primitives::Address,
    serde::{Deserialize, Serialize},
    std::{fmt::Debug, num::NonZeroUsize},
    url::Url,
};

fn default_max_cache_size() -> NonZeroUsize {
    // Note that this default value does not apply to both the orderbook and
    // autopilot! Remember to explicitly change it in the infra repo.
    NonZeroUsize::new(10000).unwrap()
}

/// Addresses banned from creating orders, with a local cache.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BannedUsersConfig {
    /// List of account addresses to be denied from order creation.
    #[serde(default)]
    pub addresses: Vec<Address>,

    /// Maximum number of entries to keep in the banned users cache.
    #[serde(default = "default_max_cache_size")]
    pub max_cache_size: NonZeroUsize,

    /// Optional Hermod (zeroShadow) sanctioned address checker.
    #[serde(default)]
    pub hermod: Option<HermodConfig>,
}

impl Default for BannedUsersConfig {
    fn default() -> Self {
        Self {
            addresses: Vec::new(),
            max_cache_size: default_max_cache_size(),
            hermod: None,
        }
    }
}

/// Hermod is zeroShadow's self-hosted sanctioned-address checker. Queries
/// are made against an HMAC-SHA256 obfuscated form of the address using a
/// per-customer key.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HermodConfig {
    /// Base URL of the Hermod agent (e.g. `http://hermod:3000`).
    pub url: Url,

    /// Per-customer HMAC key used to obfuscate addresses before sending.
    #[serde(deserialize_with = "deserialize_string_from_env")]
    pub hmac_key: String,

    /// Optional API key sent as a Bearer token, if the agent was started
    /// with `API_KEY` set.
    #[serde(default, deserialize_with = "deserialize_optional_string_from_env")]
    pub api_key: Option<String>,
}

impl Debug for HermodConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HermodConfig")
            .field("url", &self.url)
            .field("hmac_key", &"<REDACTED>")
            .field("api_key", &self.api_key.as_ref().map(|_| "<REDACTED>"))
            .finish()
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
        assert!(config.hermod.is_none());
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

    #[test]
    fn deserialize_with_hermod() {
        let toml = r#"
        [hermod]
        url = "http://hermod:3000"
        hmac-key = "key"
        api-key = "secret"
        "#;
        let config: BannedUsersConfig = toml::from_str(toml).unwrap();
        let hermod = config.hermod.unwrap();
        assert_eq!(hermod.url.as_str(), "http://hermod:3000/");
        assert_eq!(hermod.hmac_key, "key");
        assert_eq!(hermod.api_key.as_deref(), Some("secret"));
    }

    #[test]
    fn hermod_secrets_redacted() {
        let config = HermodConfig {
            url: "http://hermod:3000".parse().unwrap(),
            hmac_key: "hmac-secret-value".to_string(),
            api_key: Some("api-secret-value".to_string()),
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains(r#"hmac_key: "<REDACTED>""#));
        assert!(debug.contains(r#"api_key: Some("<REDACTED>")"#));
        assert!(!debug.contains("hmac-secret-value"));
        assert!(!debug.contains("api-secret-value"));
    }

    #[test]
    fn hermod_secrets_from_env() {
        let hmac_var = "TEST_HERMOD_HMAC_KEY";
        let api_var = "TEST_HERMOD_API_KEY";
        // SAFETY: no other threads access these env vars.
        unsafe { std::env::set_var(hmac_var, "env-hmac") };
        unsafe { std::env::set_var(api_var, "env-api") };

        let toml = format!(
            r#"
            [hermod]
            url = "http://hermod:3000"
            hmac-key = "%{hmac_var}"
            api-key = "%{api_var}"
            "#,
        );
        let config: BannedUsersConfig = toml::from_str(&toml).unwrap();
        let hermod = config.hermod.unwrap();
        assert_eq!(hermod.hmac_key, "env-hmac");
        assert_eq!(hermod.api_key.as_deref(), Some("env-api"));

        // SAFETY: no other threads access these env vars.
        unsafe { std::env::remove_var(hmac_var) };
        unsafe { std::env::remove_var(api_var) };
    }
}
