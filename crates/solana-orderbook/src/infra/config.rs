//! Configuration of infrastructural components.

use {
    configs::{database::DatabasePoolConfig, shared::LoggingConfig},
    serde::Deserialize,
    std::{net::SocketAddr, path::Path, time::Duration},
    tokio::fs,
    url::Url,
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
    /// Connection configuration for the database the indexer writes to. The
    /// orderbook only reads, so the read URL wins when configured.
    #[serde(default)]
    pub database: DatabasePoolConfig,
    /// HTTP API server configuration.
    pub http: Http,
    /// Quote endpoint configuration.
    pub quoting: Quoting,
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

/// Quote endpoint configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Quoting {
    /// Base URL of the driver that quotes orders.
    pub driver_url: Url,
    /// How long the driver has to answer before the quote fails.
    #[serde(with = "humantime_serde", default = "default_quote_timeout")]
    pub timeout: Duration,
}

fn default_quote_timeout() -> Duration {
    Duration::from_secs(5)
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
        assert_eq!(config.database.write_url.as_str(), "postgresql://");
        assert!(config.database.read_url.is_none());
        assert_eq!(config.http.bind_address, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.logging.filter, "info,solana_orderbook=debug");
        assert_eq!(config.quoting.driver_url.as_str(), "http://localhost:8000/");
        assert_eq!(config.quoting.timeout, Duration::from_secs(5));
    }
}
