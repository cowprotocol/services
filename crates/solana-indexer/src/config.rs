//! Configuration of the indexer's external endpoints.

use {
    configs::shared::LoggingConfig,
    serde::Deserialize,
    serde_ext::{deserialize_optional_solana_pubkey_b58, deserialize_solana_pubkey_b58},
    solana_sdk::pubkey::Pubkey,
    std::{path::Path, time::Duration},
    tokio::fs,
};

/// Load the indexer configuration from a TOML file.
///
/// # Panics
///
/// This function panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));

    toml::de::from_str(&data).unwrap_or_else(|err| {
        if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") {
            panic!("failed to parse TOML config at {path:?}: {err:#?}")
        } else {
            panic!(
                "failed to parse TOML config at: {path:?}. Set TOML_TRACE_ERROR=1 to print \
                 parsing error but this may leak secrets."
            )
        }
    })
}

/// Configuration of the indexer's external endpoints.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// Postgres connection URL.
    #[serde(default = "default_db_url")]
    pub db_url: String,
    /// Chain and deployment-specific configuration.
    pub chain: Chain,
    /// Yellowstone gRPC stream configuration.
    pub yellowstone: Yellowstone,
    /// JSON-RPC client configuration.
    pub rpc: Rpc,
    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    /// Build the `observe::Config` for the tracing framework from the logging
    /// configuration.
    pub fn observe_config(&self) -> observe::Config {
        observe::Config::new(
            &self.logging.filter,
            self.logging.stderr_threshold,
            self.logging.use_json,
            None,
        )
    }
}

fn default_db_url() -> String {
    "postgresql://".to_owned()
}

/// Solana chain configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Chain {
    /// On-chain program id of the settlement contract.
    #[serde(deserialize_with = "deserialize_solana_pubkey_b58")]
    pub settlement_program_id: Pubkey,
    /// On-chain program id of SolFlow. Absent until the program exists.
    #[serde(default, deserialize_with = "deserialize_optional_solana_pubkey_b58")]
    pub solflow_program_id: Option<Pubkey>,
}

/// Yellowstone gRPC stream configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Yellowstone {
    /// gRPC endpoint, `https` endpoints get TLS.
    pub endpoint: url::Url,
    /// Provider authentication token, sent as the x-token header.
    pub x_token: Option<String>,
}

/// JSON-RPC client configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Rpc {
    /// RPC endpoint to connect to.
    pub endpoint: url::Url,
    /// Timeout for individual RPC requests.
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The example config stays parseable.
    #[tokio::test]
    async fn load_example_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("example.toml");
        let config = super::load(&path).await;
        assert!(config.chain.solflow_program_id.is_none());
        assert_eq!(config.logging.filter, "info,solana_indexer=debug");
    }
}
