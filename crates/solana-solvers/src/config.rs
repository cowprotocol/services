//! Solver-engine configuration.

use {
    configs::rate_limit::Strategy,
    serde::Deserialize,
    std::{num::NonZeroUsize, path::Path, time::Duration},
    url::Url,
};

/// Jupiter solver configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// Orders quoted at once. Each quote is two sequential Jupiter requests,
    /// three when the engine has to price the buy token itself.
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

    pub dex: JupiterConfig,
}

impl Config {
    /// The back-off applied to rate-limited Jupiter responses.
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
    /// 50 = 0.5%.
    pub slippage_bps: u16,

    /// Serve buy orders via ExactOut swaps. Off by default.
    #[serde(default)]
    pub enable_buy_orders: bool,
}

/// Load and parse the TOML config file.
///
/// # Panics
///
/// Panics on I/O or parse errors: a bad config is a startup failure.
pub async fn load(path: &Path) -> Config {
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
        let config: Config =
            toml::from_str(include_str!("../config/example.jupiter.toml")).unwrap();
        assert_eq!(config.dex.endpoint.as_str(), "https://api.jup.ag/");
        assert_eq!(config.dex.slippage_bps, 50);
        assert!(config.dex.api_key.is_some());
        assert!(!config.dex.enable_buy_orders);
        assert_eq!(config.concurrent_requests.get(), 8);
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
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.concurrent_requests.get(), 8);
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
        assert!(toml::from_str::<Config>(toml).is_err());
    }
}
