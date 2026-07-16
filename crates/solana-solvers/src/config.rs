//! Solver-engine configuration.
//!
//! Mirrors the `crates/solvers` config shape: a `[dex]` table with a
//! per-aggregator sub-table. Jupiter is the only backend at MVP.

use {serde::Deserialize, std::path::Path, url::Url};

/// Top-level configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub dex: DexConfig,
}

/// The `[dex]` table. One sub-table per aggregator backend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DexConfig {
    pub jupiter: JupiterConfig,
}

/// The `[dex.jupiter]` sub-table.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JupiterConfig {
    /// Base URL of the Jupiter (or Triton-hosted) swap API.
    pub endpoint: Url,

    /// API key. `None` for the public API, set for Triton's hosted Metis.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Slippage tolerance encoded into the quote request, as a percent string.
    pub slippage: String,

    /// Whether buy orders (Jupiter `ExactOut`) are served. Off by default.
    #[serde(default)]
    pub enable_buy_orders: bool,

    /// Default compute-unit estimate used when a quote response carries none.
    pub cu_default: u32,
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
        assert_eq!(config.dex.jupiter.endpoint.as_str(), "https://api.jup.ag/");
        assert_eq!(config.dex.jupiter.slippage, "0.5");
        assert!(!config.dex.jupiter.enable_buy_orders);
        assert_eq!(config.dex.jupiter.cu_default, 200_000);
        assert!(config.dex.jupiter.api_key.is_none());
    }

    #[test]
    fn rejects_unknown_keys() {
        let toml = r#"
[dex.jupiter]
endpoint = "https://api.jup.ag"
slippage = "0.5"
cu-default = 200000
bogus = true
"#;
        assert!(toml::from_str::<Config>(toml).is_err());
    }
}
