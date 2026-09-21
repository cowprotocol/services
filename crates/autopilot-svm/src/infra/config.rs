//! Configuration of the autopilot's endpoints and competition parameters.

use {
    configs::{database::DatabasePoolConfig, shared::LoggingConfig},
    serde::Deserialize,
    serde_ext::{deserialize_nonempty_vec, deserialize_solana_pubkey_b58},
    solana_sdk::pubkey::Pubkey,
    std::{num::NonZero, path::Path, time::Duration},
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
    /// On-chain addresses the autopilot reads.
    pub contracts: Contracts,
    /// Competition parameters.
    #[serde(default)]
    pub competition: Competition,
    /// Port the metrics and probes server binds to, on every interface.
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    /// If no auction cycle completed in this time the pod fails the liveness
    /// check.
    #[serde(with = "humantime_serde", default = "default_max_auction_age")]
    pub max_auction_age: Duration,
    /// Minimum time between auction cycles. Zero runs a cycle every new slot.
    #[serde(with = "humantime_serde", default = "default_min_auction_interval")]
    pub min_auction_interval: Duration,
    /// Slots the indexer may lag behind the tip before auction cuts are
    /// skipped, so a stalled indexer stops feeding stale orders to solvers.
    #[serde(default = "default_max_indexer_lag_slots")]
    pub max_indexer_lag_slots: u64,
    /// The driver endpoints participating in every auction.
    #[serde(deserialize_with = "deserialize_nonempty_vec")]
    pub drivers: Vec<Driver>,
    /// Sponsored order execution. Must be set when the orderbook accepts
    /// sponsored orders: without it their winning solutions dispatch without
    /// creations and fail at the driver.
    pub sponsoring: Option<Sponsoring>,
    /// Native price lookups for auction tokens. Required with no default
    /// source: pricing through a third party is a deployment decision,
    /// never a silent fallback.
    pub native_prices: NativePrices,
    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Native price lookups.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NativePrices {
    /// Price sources in fallback order: a token the first one does not price
    /// is asked from the next. Empty, the autopilot prices every token at the
    /// native denominator, so scores compare raw surplus. Pricing through a
    /// third party stays a deployment decision, there is no default source.
    #[serde(default)]
    pub estimators: Vec<NativePriceEstimator>,
    /// How long a fetched price serves auctions before it is refetched.
    #[serde(with = "humantime_serde", default = "default_prices_ttl")]
    pub ttl: Duration,
    /// Lamports a driver source buys per probe quote. The probe is
    /// denominated in the native token so its economic size does not depend
    /// on what one whole unit of the priced token is worth.
    #[serde(default = "default_driver_probe_lamports")]
    pub driver_probe_lamports: u64,
}

/// A tenth of a SOL, the fraction the EVM chains probe with.
const fn default_driver_probe_lamports() -> u64 {
    100_000_000
}

/// One native price source.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NativePriceEstimator {
    /// The CoinGecko `simple/token_price` API.
    CoinGecko {
        /// Base URL of the CoinGecko API.
        endpoint: url::Url,
        /// API key sent with every price request as the CoinGecko Pro plan
        /// header.
        #[serde(default)]
        api_key: Option<String>,
    },
    /// A solver driver, quoted through its regular `/quote` route. The url
    /// includes the solver path, like the `[[drivers]]` entries.
    Driver { name: String, url: url::Url },
}

const fn default_prices_ttl() -> Duration {
    Duration::from_secs(30)
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

const fn default_metrics_port() -> u16 {
    observe::metrics::DEFAULT_METRICS_PORT
}

const fn default_max_auction_age() -> Duration {
    Duration::from_mins(5)
}

const fn default_min_auction_interval() -> Duration {
    Duration::ZERO
}

/// One blockhash lifetime: beyond it the freshest pending creations in the
/// stale data would already be dying.
const fn default_max_indexer_lag_slots() -> u64 {
    150
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

/// Sponsored order execution configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Sponsoring {
    /// Path to the funder keypair countersigning sponsored creation
    /// transactions. TODO: plaintext keypair paths are temporary. Secrets
    /// must not live in the config or its repository long term.
    pub funder_keypair: std::path::PathBuf,
}

/// On-chain addresses: programs and mints.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Contracts {
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
            config.contracts.wrapped_native_mint,
            "So11111111111111111111111111111111111111112"
                .parse()
                .unwrap()
        );
        assert_eq!(config.competition.max_winners.get(), 1);
        assert_eq!(config.competition.solve_deadline, Duration::from_secs(6));
        assert_eq!(config.competition.submission_deadline_slots.get(), 25);
        assert_eq!(config.max_auction_age, Duration::from_secs(5 * 60));
        assert_eq!(config.min_auction_interval, Duration::from_secs(2));
        assert_eq!(config.max_indexer_lag_slots, 150);
        assert_eq!(config.native_prices.ttl, Duration::from_secs(30));
        assert_eq!(config.native_prices.driver_probe_lamports, 100_000_000);
        assert!(matches!(
            &config.native_prices.estimators[..],
            [
                NativePriceEstimator::CoinGecko { endpoint, api_key: None },
                NativePriceEstimator::Driver { name, .. },
            ] if endpoint.as_str() == "https://api.coingecko.com/api/v3/" && name == "baseline"
        ));
        assert_eq!(config.drivers.len(), 1);
        assert_eq!(config.drivers[0].name, "baseline");
        assert_eq!(config.logging.filter, "info,autopilot_svm=debug");
    }
}
