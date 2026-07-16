//! Solver-engine configuration.

use {serde::Deserialize, std::path::Path, url::Url};

/// Jupiter solver configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub dex: JupiterConfig,
}

/// The `[dex]` table for the Jupiter backend. The subcommand selects the
/// engine, so there is no per-aggregator sub-table.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JupiterConfig {
    /// Base URL of the Jupiter swap API (`api.jup.ag`) or a Triton-hosted Metis
    /// endpoint.
    pub endpoint: Url,

    /// API key for the Jupiter API. Required for `api.jup.ag` (issued by the
    /// Jupiter developer portal) and for Triton. Omit only for the keyless
    /// `lite-api.jup.ag` endpoint.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Slippage tolerance encoded into each quote request, as a percent string.
    pub slippage: String,

    /// Whether buy orders (Jupiter `ExactOut`) are served. Off by default.
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
    use super::*;

    #[test]
    fn parses_example_config() {
        let config: Config =
            toml::from_str(include_str!("../config/example.jupiter.toml")).unwrap();
        assert_eq!(config.dex.endpoint.as_str(), "https://api.jup.ag/");
        assert_eq!(config.dex.slippage, "0.5");
        assert!(!config.dex.enable_buy_orders);
        assert!(config.dex.api_key.is_some());
    }

    #[test]
    fn rejects_unknown_keys() {
        let toml = r#"
[dex]
endpoint = "https://api.jup.ag"
slippage = "0.5"
bogus = true
"#;
        assert!(toml::from_str::<Config>(toml).is_err());
    }
}
