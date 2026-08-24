//! Configuration of the autopilot's endpoints and competition parameters.

use {
    configs::{database::DatabasePoolConfig, shared::LoggingConfig},
    serde::Deserialize,
    serde_ext::{deserialize_nonempty_vec, deserialize_solana_pubkey_b58},
    solana_sdk::pubkey::Pubkey,
    std::{net::SocketAddr, num::NonZero, path::Path, time::Duration},
    tokio::fs,
};

/// Load the autopilot configuration from a TOML file.
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

/// Configuration of the autopilot's endpoints and competition parameters.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// Connection configuration for the database the indexer writes to.
    #[serde(default)]
    pub database: DatabasePoolConfig,
    /// JSON-RPC client configuration.
    pub rpc: Rpc,
    /// Chain and deployment-specific configuration.
    pub chain: Chain,
    /// Competition parameters.
    #[serde(default)]
    pub competition: Competition,
    /// Address the metrics and probes server binds to.
    #[serde(default = "default_metrics_address")]
    pub metrics_address: SocketAddr,
    /// If no auction cycle completed in this time the pod fails the liveness
    /// check.
    #[serde(with = "humantime_serde", default = "default_max_auction_age")]
    pub max_auction_age: Duration,
    /// The driver endpoints participating in every auction.
    #[serde(deserialize_with = "deserialize_nonempty_vec")]
    pub drivers: Vec<Driver>,
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

fn default_metrics_address() -> SocketAddr {
    "0.0.0.0:9588".parse().expect("valid address literal")
}

const fn default_max_auction_age() -> Duration {
    Duration::from_secs(5 * 60)
}

/// JSON-RPC client configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Rpc {
    /// HTTP endpoint of the Solana JSON-RPC node.
    pub endpoint: url::Url,
    /// Timeout for a single RPC request.
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
}

/// Solana chain configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Chain {
    /// The wrapped native token mint (wSOL), the unit scores are denominated
    /// in.
    #[serde(deserialize_with = "deserialize_solana_pubkey_b58")]
    pub wrapped_native_mint: Pubkey,
}

/// Competition parameters.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Competition {
    /// Maximum number of winning solutions per auction.
    pub max_winners: NonZero<usize>,
    /// How long drivers get to answer `/solve`.
    #[serde(with = "humantime_serde")]
    pub solve_deadline: Duration,
    /// Slots a settlement may take after ranking before it counts as late.
    pub submission_deadline_slots: NonZero<u64>,
}

impl Default for Competition {
    fn default() -> Self {
        Self {
            max_winners: NonZero::new(1).expect("non-zero literal"),
            // The EVM fast chains run 6s solve deadlines.
            solve_deadline: Duration::from_secs(6),
            submission_deadline_slots: NonZero::new(25).expect("non-zero literal"),
        }
    }
}

/// One driver endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Driver {
    /// Name for logs and metrics.
    pub name: String,
    /// HTTP endpoint of the driver API.
    pub url: url::Url,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loads_the_example_config() {
        let config = load(std::path::Path::new("example.toml")).await;
        assert_eq!(config.database.write_url.as_str(), "postgresql://");
        assert_eq!(
            config.chain.wrapped_native_mint,
            "So11111111111111111111111111111111111111112"
                .parse()
                .unwrap()
        );
        assert_eq!(config.competition.max_winners.get(), 1);
        assert_eq!(config.competition.solve_deadline, Duration::from_secs(6));
        assert_eq!(config.competition.submission_deadline_slots.get(), 25);
        assert_eq!(config.max_auction_age, Duration::from_secs(5 * 60));
        assert_eq!(config.drivers.len(), 1);
        assert_eq!(config.drivers[0].name, "baseline");
        assert_eq!(config.logging.filter, "info,autopilot_svm=debug");
    }
}
