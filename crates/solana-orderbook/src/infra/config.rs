//! Configuration of infrastructural components.

use {
    crate::infra::api::QuoteLimits,
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
    /// Base URL of the driver asked to quote orders. A missing trailing slash
    /// is added: routes resolve relative to this, so without it the last path
    /// segment would be replaced rather than extended.
    #[serde(deserialize_with = "serde_ext::deserialize_url_with_trailing_slash")]
    pub driver_url: Url,
    /// How long the driver has to answer before the quote fails.
    #[serde(with = "humantime_serde", default = "default_quote_timeout")]
    pub timeout: Duration,
    /// Least far in the future a quoted order's `validTo` may lie.
    #[serde(with = "humantime_serde", default = "default_min_validity")]
    pub min_validity: Duration,
    /// Furthest in the future a quoted order's `validTo` may lie.
    #[serde(with = "humantime_serde", default = "default_max_validity")]
    pub max_validity: Duration,
    /// How long the quoted amounts are honored.
    #[serde(with = "humantime_serde", default = "default_quote_expiry")]
    pub quote_expiry: Duration,
}

impl Quoting {
    /// The validity bounds and expiry as the API consumes them.
    pub fn limits(&self) -> QuoteLimits {
        QuoteLimits {
            min_validity: self.min_validity,
            max_validity: self.max_validity,
            quote_expiry: self.quote_expiry,
        }
    }
}

fn default_quote_timeout() -> Duration {
    Duration::from_secs(5)
}

fn default_min_validity() -> Duration {
    QuoteLimits::default().min_validity
}

fn default_max_validity() -> Duration {
    QuoteLimits::default().max_validity
}

fn default_quote_expiry() -> Duration {
    QuoteLimits::default().quote_expiry
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
        assert_eq!(config.quoting.min_validity, Duration::from_secs(120));
        assert_eq!(config.quoting.max_validity, Duration::from_secs(7200));
        assert_eq!(config.quoting.quote_expiry, Duration::from_secs(60));
    }

    /// Routes resolve relative to `driver-url`, so a base URL with a path
    /// keeps it instead of having its last segment replaced.
    #[test]
    fn driver_url_gets_a_trailing_slash() {
        #[derive(Debug, Deserialize)]
        struct Wrapper {
            quoting: Quoting,
        }
        let wrapper: Wrapper = toml::de::from_str(
            r#"
            [quoting]
            driver-url = "http://driver/baseline"
            "#,
        )
        .unwrap();
        assert_eq!(
            wrapper.quoting.driver_url.as_str(),
            "http://driver/baseline/"
        );
        assert_eq!(
            wrapper.quoting.driver_url.join("quote").unwrap().as_str(),
            "http://driver/baseline/quote"
        );
    }
}
