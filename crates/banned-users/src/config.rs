//! Configuration of the banned user detection. All services read the same
//! section; only the address type changes.

use {
    serde::Deserialize,
    serde_ext::Env,
    serde_with::{DisplayFromStr, serde_as},
    std::{fmt::Debug, num::NonZeroUsize, time::Duration},
    url::Url,
};

fn default_refresh_interval() -> Duration {
    Duration::from_secs(5)
}

fn default_max_idle() -> Duration {
    Duration::from_secs(60)
}

fn default_max_cache_size() -> NonZeroUsize {
    // Note that this default value does not apply to both the orderbook and
    // autopilot! Remember to explicitly change it in the infra repo.
    NonZeroUsize::new(10000).unwrap()
}

/// Addresses banned from creating orders, with a local cache. An address is
/// read from its text form, for example `0x` hex or base58.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(
    rename_all = "kebab-case",
    deny_unknown_fields,
    bound(
        deserialize = "A: std::str::FromStr, A::Err: std::fmt::Display",
        serialize = "A: std::fmt::Display"
    )
)]
pub struct Config<A> {
    /// List of account addresses to be denied from order creation.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub addresses: Vec<A>,

    /// Maximum number of addresses to cache for Hermod: their signatures
    /// (HMACs) and their verdicts.
    #[serde(default = "default_max_cache_size")]
    pub max_cache_size: NonZeroUsize,

    /// Optional Hermod (zeroShadow) sanctioned address checker.
    #[serde(default)]
    pub hermod: Option<HermodConfig>,
}

impl<A> Default for Config<A> {
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
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HermodConfig {
    /// Base URL of the Hermod agent (e.g. `http://hermod:3000`).
    pub url: Url,

    /// Per-customer HMAC key used to obfuscate addresses before sending.
    pub hmac_key: Env<String, true>,

    /// Optional API key sent as a Bearer token, if the agent was started
    /// with `API_KEY` set.
    pub api_key: Option<Env<String, true>>,

    /// How often the cached addresses are checked again.
    #[serde(with = "humantime_serde", default = "default_refresh_interval")]
    pub refresh_interval: Duration,

    /// An address that nobody requested for this long leaves the cache.
    #[serde(with = "humantime_serde", default = "default_max_idle")]
    pub max_idle: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests read the addresses as text: the callers choose the type.
    type Config = super::Config<String>;

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: Config = toml::from_str(toml).unwrap();
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
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.addresses,
            ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        );
        assert_eq!(config.max_cache_size.get(), 5000);
    }

    #[test]
    fn rejects_an_address_that_does_not_parse() {
        let toml = r#"addresses = ["not an address"]"#;
        assert!(toml::from_str::<super::Config<std::net::Ipv4Addr>>(toml).is_err());
    }

    #[test]
    fn serialized_config_deserializes_again() {
        let config = Config {
            addresses: vec!["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()],
            max_cache_size: NonZeroUsize::new(5000).unwrap(),
            hermod: None,
        };
        let deserialized: Config = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(deserialized.addresses, config.addresses);
        assert_eq!(deserialized.max_cache_size, config.max_cache_size);
    }

    #[test]
    fn deserialize_with_hermod() {
        let toml = r#"
        [hermod]
        url = "http://hermod:3000"
        hmac-key = "key"
        api-key = "secret"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        let hermod = config.hermod.unwrap();
        assert_eq!(hermod.url.as_str(), "http://hermod:3000/");
        assert_eq!(hermod.hmac_key.into_inner(), "key");
        assert_eq!(hermod.api_key.unwrap().into_inner(), "secret");
        assert_eq!(hermod.refresh_interval, Duration::from_secs(5));
        assert_eq!(hermod.max_idle, Duration::from_secs(60));
    }

    #[test]
    fn deserialize_hermod_cache_timing() {
        let toml = r#"
        [hermod]
        url = "http://hermod:3000"
        hmac-key = "key"
        refresh-interval = "12s"
        max-idle = "2m"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        let hermod = config.hermod.unwrap();
        assert_eq!(hermod.refresh_interval, Duration::from_secs(12));
        assert_eq!(hermod.max_idle, Duration::from_secs(120));
    }

    #[test]
    fn hermod_secrets_redacted() {
        let config = HermodConfig {
            url: "http://hermod:3000".parse().unwrap(),
            hmac_key: Env("hmac-secret-value".to_string()),
            api_key: Some(Env("api-secret-value".to_string())),
            refresh_interval: default_refresh_interval(),
            max_idle: default_max_idle(),
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains(r#"hmac_key: "<REDACTED>""#));
        assert!(debug.contains(r#"api_key: Some("<REDACTED>")"#));
        assert!(!debug.contains("hmac-secret-value"));
        assert!(!debug.contains("api-secret-value"));
    }

    /// An `api-key` that names an unset environment variable is an error.
    #[test]
    fn hermod_api_key_from_unset_env_var_is_error() {
        let toml = r#"
        [hermod]
        url = "http://hermod:3000"
        hmac-key = "key"
        api-key = "%NONEXISTENT_TEST_HERMOD_API_KEY"
        "#;
        let err = toml::from_str::<Config>(toml).unwrap_err();
        assert!(
            err.to_string().contains("NONEXISTENT_TEST_HERMOD_API_KEY"),
            "{err}"
        );
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
        let config: Config = toml::from_str(&toml).unwrap();
        let hermod = config.hermod.unwrap();
        assert_eq!(hermod.hmac_key.into_inner(), "env-hmac");
        assert_eq!(hermod.api_key.unwrap().into_inner(), "env-api");

        // SAFETY: no other threads access these env vars.
        unsafe { std::env::remove_var(hmac_var) };
        unsafe { std::env::remove_var(api_var) };
    }
}
