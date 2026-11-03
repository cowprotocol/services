//! Solver-engine configuration.

use {
    configs::rate_limit::Strategy,
    serde::{Deserialize, de::DeserializeOwned},
    std::{num::NonZeroUsize, path::Path, time::Duration},
    url::Url,
};

/// Solver configuration around the engine's `[dex]` table.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config<D> {
    /// Orders quoted at once.
    #[serde(default = "default_concurrent_requests")]
    pub concurrent_requests: NonZeroUsize,

    /// Multiplier on the pause after each consecutive rate-limited response.
    #[serde(default = "default_back_off_growth_factor")]
    pub back_off_growth_factor: f64,

    /// Pause after the first rate-limited response.
    #[serde(with = "humantime_serde", default = "default_min_back_off")]
    pub min_back_off: Duration,

    /// Longest pause the growth factor reaches.
    #[serde(with = "humantime_serde", default = "default_max_back_off")]
    pub max_back_off: Duration,

    /// The most orders one batched solution fills: per side and directed
    /// pair, orders are quoted most permissive first in batches of up to
    /// this many, each its own solution. 1 quotes every order on its own.
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: NonZeroUsize,

    pub dex: D,
}

impl<D> Config<D> {
    /// The back-off applied to rate-limited DEX responses.
    ///
    /// # Panics
    ///
    /// Panics on inverted back-off bounds or a growth factor below one: a bad
    /// config is a startup failure.
    pub fn rate_limiting(&self) -> Strategy {
        Strategy::try_new(
            self.back_off_growth_factor,
            self.min_back_off,
            self.max_back_off,
        )
        .unwrap_or_else(|err| panic!("rate limiting config: {err}"))
    }
}

fn default_concurrent_requests() -> NonZeroUsize {
    NonZeroUsize::new(8).unwrap()
}

fn default_back_off_growth_factor() -> f64 {
    2.0
}

fn default_min_back_off() -> Duration {
    Duration::from_secs(1)
}

fn default_max_back_off() -> Duration {
    Duration::from_secs(8)
}

fn default_max_batch_size() -> NonZeroUsize {
    NonZeroUsize::new(1).unwrap()
}

/// The `[dex]` table for the Jupiter backend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JupiterConfig {
    /// Base URL of the Jupiter swap API: `api.jup.ag`, or a Triton-hosted
    /// endpoint.
    pub endpoint: Url,

    /// API key from the Jupiter developer portal (or Triton). Requests work
    /// without one but are heavily rate-limited, so set it for production.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Slippage tolerance in basis points, sent to Jupiter as `slippageBps`.
    /// We bid Jupiter's `otherAmountThreshold`, so this is how far below its
    /// expected output (sells) or above its expected input (buys) the bids
    /// land. 50 = 0.5%.
    pub slippage_bps: u16,

    /// Serve buy orders via ExactOut swaps. Off by default.
    #[serde(default)]
    pub enable_buy_orders: bool,
}

/// The `[dex]` table for the Bitget backend. No `Debug`: it holds the secret.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BitgetConfig {
    /// Base URL of the Bitget swap API, or a proxy route to it.
    #[serde(deserialize_with = "serde_ext::deserialize_url_with_trailing_slash")]
    pub endpoint: Url,

    pub api_key: String,

    pub api_secret: String,

    /// Partner code sent in the `Partner-Code` header.
    #[serde(default = "default_partner_code")]
    pub partner_code: String,

    /// Slippage tolerance in basis points. We bid Bitget's minimum output, so
    /// this is how far below its expected output the bids land.
    pub slippage_bps: u16,

    /// Caps the accounts Bitget's route may use, leaving room for the
    /// settlement around it in the 64 a transaction can lock.
    #[serde(default = "default_max_accounts")]
    pub max_accounts: u16,
}

fn default_max_accounts() -> u16 {
    48
}

fn default_partner_code() -> String {
    "cowswap".to_string()
}

/// Load and parse the TOML config file.
///
/// # Panics
///
/// Panics on I/O or parse errors: a bad config is a startup failure.
pub async fn load<D: DeserializeOwned>(path: &Path) -> Config<D> {
    let text = tokio::fs::read_to_string(path)
        .await
        .unwrap_or_else(|err| panic!("read config {}: {err}", path.display()));
    toml::from_str(&text).unwrap_or_else(|err| panic!("parse config {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use {super::*, std::time::Duration};

    #[test]
    fn parses_example_config() {
        let config: Config<JupiterConfig> =
            toml::from_str(include_str!("../config/example.jupiter.toml")).unwrap();
        assert_eq!(config.dex.endpoint.as_str(), "https://api.jup.ag/");
        assert_eq!(config.dex.slippage_bps, 50);
        assert!(config.dex.api_key.is_some());
        assert!(!config.dex.enable_buy_orders);
        assert_eq!(config.concurrent_requests.get(), 8);
        assert_eq!(config.max_batch_size.get(), 4);
        let strategy = config.rate_limiting();
        assert_eq!(strategy.back_off_growth_factor, 2.0);
        assert_eq!(strategy.min_back_off, Duration::from_secs(1));
        assert_eq!(strategy.max_back_off, Duration::from_secs(8));
    }

    #[test]
    fn defaults_the_throttle() {
        let toml = r#"
[dex]
endpoint = "https://api.jup.ag"
slippage-bps = 50
"#;
        let config: Config<JupiterConfig> = toml::from_str(toml).unwrap();
        assert_eq!(config.concurrent_requests.get(), 8);
        assert_eq!(config.max_batch_size.get(), 1);
        let strategy = config.rate_limiting();
        assert_eq!(strategy.back_off_growth_factor, 2.0);
        assert_eq!(strategy.min_back_off, Duration::from_secs(1));
        assert_eq!(strategy.max_back_off, Duration::from_secs(8));
    }

    #[test]
    fn rejects_unknown_keys() {
        let toml = r#"
[dex]
endpoint = "https://api.jup.ag"
slippage-bps = 50
bogus = true
"#;
        assert!(toml::from_str::<Config<JupiterConfig>>(toml).is_err());
    }

    #[test]
    fn parses_bitget_example_config() {
        let config: Config<BitgetConfig> =
            toml::from_str(include_str!("../config/example.bitget.toml")).unwrap();
        assert_eq!(
            config.dex.endpoint.as_str(),
            "https://bopenapi.bgwapi.io/bgw-pro/swapx/pro/"
        );
        assert_eq!(config.dex.slippage_bps, 20);
        assert_eq!(config.dex.max_accounts, 48);
        assert_eq!(config.dex.partner_code, "cowswap");
    }

    #[test]
    fn requires_bitget_credentials() {
        // Deploy templating drops `key = ""` lines, so a missing secret must
        // fail to parse.
        let toml = r#"
[dex]
endpoint = "https://bopenapi.bgwapi.io/bgw-pro/swapx/pro/"
api-key = "key"
slippage-bps = 50
"#;
        assert!(toml::from_str::<Config<BitgetConfig>>(toml).is_err());
    }
}
