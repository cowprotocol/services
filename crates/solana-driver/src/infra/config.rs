//! Configuration of infrastructural components.

use {
    configs::shared::LoggingConfig,
    serde::Deserialize,
    serde_ext::{
        deserialize_nonempty_vec,
        deserialize_solana_pubkey_b58,
        deserialize_url_with_trailing_slash,
    },
    solana_sdk::pubkey::Pubkey,
    std::{
        net::SocketAddr,
        num::NonZero,
        path::{Path, PathBuf},
        time::Duration,
    },
    tokio::fs,
};

/// Load the driver configuration from a TOML file.
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
    /// Chain and deployment-specific configuration.
    pub chain: Chain,
    /// RPC client configuration.
    pub rpc: Rpc,
    /// HTTP API server configuration.
    pub http: Http,
    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,
    /// Settlement submission configuration.
    #[serde(default)]
    pub settlement: Settlement,
    /// Configured solver engines to query for solutions.
    #[serde(deserialize_with = "deserialize_nonempty_vec")]
    pub solvers: Vec<Solver>,
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

fn default_settlement_program_id() -> Pubkey {
    cow_settlement_interface::ID
}

/// Solana chain configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Chain {
    /// On-chain program id of the settlement contract. Defaults to the
    /// official deployment the interface crate exports.
    #[serde(
        default = "default_settlement_program_id",
        deserialize_with = "deserialize_solana_pubkey_b58"
    )]
    pub settlement_program_id: Pubkey,
}

/// RPC client configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Rpc {
    /// RPC endpoint to connect to.
    pub endpoint: url::Url,
    /// Timeout for individual RPC requests.
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
}

/// HTTP API server configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Http {
    /// Address the HTTP API server binds to and listens on.
    pub bind_address: SocketAddr,
}

/// A configured solver engine.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Solver {
    /// Human-readable name identifying this solver, used for logging and
    /// metrics.
    pub name: String,
    /// HTTP endpoint of the solver engine API.
    #[serde(deserialize_with = "deserialize_url_with_trailing_slash")]
    pub endpoint: url::Url,
    /// Path to the solver's settlement signer keypair. The driver's on-chain
    /// identity for this solver is derived from this keypair.
    ///
    /// TODO: plaintext keypair paths are temporary. Secrets must not live in
    /// plaintext config long-term; KMS-backed signers are planned, mirroring
    /// the EVM driver's `submission_accounts`.
    pub signer_keypair: PathBuf,
    /// Temporary staging knob: solve only auctions whose id is a multiple of
    /// this value and sit the rest out, so other solvers win settlements to
    /// test against. Absent means every auction.
    #[serde(default)]
    pub solve_every_nth_auction: Option<NonZero<u64>>,
}

/// Settlement submission configuration.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Settlement {
    /// Push reductions, in basis points of the promised buy amount, a
    /// settlement may fall back to when the solver's swap delivers less than
    /// promised. They are simulated alongside the promise and the smallest
    /// passing one is submitted, never below the order's limit price. Must be
    /// strictly ascending; empty disables the fallback.
    #[serde(default, deserialize_with = "deserialize_push_reduction_bps")]
    pub push_reduction_bps: Vec<u16>,
}

fn deserialize_push_reduction_bps<'de, D>(deserializer: D) -> Result<Vec<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bps: Vec<u16> = serde::Deserialize::deserialize(deserializer)?;
    if let Some(out_of_range) = bps.iter().find(|bps| !(1..=10_000).contains(*bps)) {
        return Err(serde::de::Error::custom(format!(
            "push reduction of {out_of_range} bps is outside 1..=10000"
        )));
    }
    if !bps.is_sorted_by(|a, b| a < b) {
        return Err(serde::de::Error::custom(
            "push reductions must be strictly ascending",
        ));
    }
    Ok(bps)
}

#[cfg(test)]
mod tests {
    use {super::*, std::path::Path};

    #[tokio::test]
    async fn load_example_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("example.toml");
        let config = load(&path).await;

        assert_eq!(
            config.rpc.endpoint.as_str(),
            "https://api.mainnet-beta.solana.com/"
        );
        assert_eq!(config.solvers.len(), 1);
        assert_eq!(config.solvers[0].name, "baseline");
        assert_eq!(
            config.solvers[0].signer_keypair,
            Path::new("/path/to/keypair.json")
        );
        assert_eq!(config.logging.filter, "info,solana_driver=debug");
        assert_eq!(config.logging.stderr_threshold, None);
        assert!(!config.logging.use_json);
    }

    #[test]
    fn solver_config_parses() {
        let solver_config = r#"
            name = "baseline"
            endpoint = "http://localhost:8001"
            signer-keypair = "/path/to/keypair.json"
        "#;
        let solver: Solver = toml::de::from_str(solver_config).unwrap();
        assert_eq!(solver.name, "baseline");
        assert_eq!(solver.signer_keypair, Path::new("/path/to/keypair.json"));
    }

    #[test]
    fn push_reductions_must_be_strictly_ascending_basis_points() {
        let parse =
            |bps: &str| toml::de::from_str::<Settlement>(&format!("push-reduction-bps = {bps}"));
        assert!(parse("[1, 2, 4]").is_ok());
        assert!(parse("[]").is_ok());
        for rejected in ["[0]", "[10001]", "[2, 1]", "[1, 1]"] {
            assert!(parse(rejected).is_err(), "{rejected} must be rejected");
        }
    }

    #[test]
    fn chain_defaults_to_interface_program_id() {
        let chain: Chain = toml::de::from_str("").unwrap();
        assert_eq!(chain.settlement_program_id, cow_settlement_interface::ID);
    }
}
