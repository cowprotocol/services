//! File-based configuration loading.

use {
    crate::infra::config::{Chain, Config, Http, Rpc, Solver},
    configs::shared::LoggingConfig,
    serde::Deserialize,
    serde_ext::deserialize_nonempty_vec,
    std::{net::SocketAddr, num::NonZero, path::Path, time::Duration},
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

    let config: FileConfig = toml::de::from_str(&data).unwrap_or_else(|err| {
        if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") {
            panic!("failed to parse TOML config at {path:?}: {err:#?}")
        } else {
            panic!(
                "failed to parse TOML config at: {path:?}. Set TOML_TRACE_ERROR=1 to print \
                 parsing error but this may leak secrets."
            )
        }
    });

    Config {
        chain: Chain {
            settlement_program_id: config
                .chain
                .settlement_program_id
                .parse()
                .expect("invalid settlement program id"),
        },
        rpc: Rpc {
            endpoints: config.rpc.endpoints,
            request_timeout: config.rpc.request_timeout,
        },
        http: Http {
            bind_address: config.http.bind_address,
        },
        logging: config.logging,
        solvers: config
            .solvers
            .into_iter()
            .map(|solver| Solver {
                name: solver.name,
                endpoint: solver.endpoint,
                max_in_flight: solver.max_in_flight.get(),
            })
            .collect(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct FileConfig {
    chain: ChainConfig,
    rpc: RpcConfig,
    http: HttpConfig,
    #[serde(default)]
    logging: LoggingConfig,
    #[serde(deserialize_with = "deserialize_nonempty_vec")]
    solvers: Vec<SolverConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ChainConfig {
    settlement_program_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct RpcConfig {
    #[serde(deserialize_with = "deserialize_nonempty_vec")]
    endpoints: Vec<url::Url>,
    #[serde(with = "humantime_serde")]
    request_timeout: Duration,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct HttpConfig {
    bind_address: SocketAddr,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SolverConfig {
    name: String,
    endpoint: url::Url,
    max_in_flight: NonZero<usize>,
}

#[cfg(test)]
mod tests {
    use {super::*, std::path::Path};

    #[tokio::test]
    async fn load_example_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("example.toml");
        let config = load(&path).await;

        assert_eq!(config.rpc.endpoints.len(), 1);
        assert_eq!(config.solvers.len(), 1);
        assert_eq!(config.solvers[0].name, "baseline");
        assert_eq!(config.solvers[0].max_in_flight, 1);
        assert_eq!(config.logging.filter, "info,solana_driver=debug");
        assert_eq!(config.logging.stderr_threshold, None);
        assert!(!config.logging.use_json);
    }

    #[test]
    fn zero_max_in_flight_rejected() {
        let solver_config = r#"
            name = "baseline"
            endpoint = "http://localhost:8001"
            max-in-flight = 0
        "#;
        let err = toml::de::from_str::<SolverConfig>(solver_config)
            .expect_err("zero max-in-flight should be rejected");
        assert!(
            err.to_string().contains("expected a nonzero usize"),
            "unexpected error: {err}"
        );
    }
}
