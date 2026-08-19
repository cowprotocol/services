//! Configuration of infrastructural components.

use {
    configs::shared::LoggingConfig,
    serde::Deserialize,
    std::{net::SocketAddr, path::Path},
    tokio::fs,
};

/// Load the orderbook configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
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

/// Configuration of infrastructural components.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// Postgres connection URL of the database the indexer writes to.
    #[serde(default = "default_db_url")]
    pub db_url: url::Url,
    /// HTTP API server configuration.
    pub http: Http,
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

fn default_db_url() -> url::Url {
    "postgresql://".parse().expect("valid default database URL")
}

/// HTTP API server configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Http {
    /// Address the HTTP API server binds to and listens on.
    pub bind_address: SocketAddr,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loads_the_example_config() {
        let config = load(Path::new("example.toml")).await;
        assert_eq!(config.db_url.as_str(), "postgresql://");
        assert_eq!(config.http.bind_address, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.logging.filter, "info,solana_orderbook=debug");
    }
}
