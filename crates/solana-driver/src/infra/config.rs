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

/// Solana chain configuration.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Chain {
    /// On-chain program id of the settlement contract.
    #[serde(deserialize_with = "deserialize_solana_pubkey_b58")]
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
    /// The solver's on-chain identity. Reported on every `domain::Solution`
    /// produced by this engine.
    #[serde(deserialize_with = "deserialize_solana_pubkey_b58")]
    pub account: Pubkey,
    /// Path to the solver's settlement signer keypair. Its public key must
    /// equal `account`; the driver fails fast on mismatch at startup.
    ///
    /// TODO: plaintext keypair paths are temporary. Secrets must not live in
    /// plaintext config long-term; KMS-backed signers are planned, mirroring
    /// the EVM driver's `submission_accounts`.
    pub signer_keypair: PathBuf,
    /// Maximum number of concurrent solve requests kept in flight per solver.
    pub max_in_flight: NonZero<usize>,
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
        assert_eq!(config.solvers[0].max_in_flight.get(), 1);
        assert_eq!(
            config.solvers[0].account,
            "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB"
                .parse()
                .unwrap()
        );
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
            account = "11111111111111111111111111111111"
            signer-keypair = "/path/to/keypair.json"
            max-in-flight = 1
        "#;
        let solver: Solver = toml::de::from_str(solver_config).unwrap();
        assert_eq!(solver.name, "baseline");
        assert_eq!(solver.account, Pubkey::default());
        assert_eq!(solver.signer_keypair, Path::new("/path/to/keypair.json"));
        assert_eq!(solver.max_in_flight.get(), 1);
    }

    #[test]
    fn zero_max_in_flight_rejected() {
        let solver_config = r#"
            name = "baseline"
            endpoint = "http://localhost:8001"
            account = "11111111111111111111111111111111"
            signer-keypair = "/path/to/keypair.json"
            max-in-flight = 0
        "#;
        let err = toml::de::from_str::<Solver>(solver_config)
            .expect_err("zero max-in-flight should be rejected");
        assert!(
            err.to_string().contains("expected a nonzero usize"),
            "unexpected error: {err}"
        );
    }
}
