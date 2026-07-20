//! Solver-engine configuration.

use {serde::Deserialize, std::path::Path, url::Url};

/// Jupiter solver configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub dex: JupiterConfig,
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
    use super::*;

    #[test]
    fn parses_example_config() {
        let config: Config =
            toml::from_str(include_str!("../config/example.jupiter.toml")).unwrap();
        assert_eq!(config.dex.endpoint.as_str(), "https://api.jup.ag/");
        assert_eq!(config.dex.slippage_bps, 50);
        assert!(config.dex.api_key.is_some());
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
