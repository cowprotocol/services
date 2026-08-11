//! File-based configuration loading.

use {
    crate::infra::config::{Chain, Config, Http, Rpc, Solver},
    serde::Deserialize,
    std::{net::SocketAddr, path::Path, time::Duration},
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

    assert!(
        !config.rpc.endpoints.is_empty(),
        "at least one RPC endpoint must be configured"
    );

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
            confirm_transaction_initial_timeout: config.rpc.confirm_transaction_initial_timeout,
        },
        http: Http {
            bind_address: config.http.bind_address,
        },
        solvers: config
            .solvers
            .into_iter()
            .map(|solver| Solver {
                name: solver.name,
                endpoint: solver.endpoint,
                max_in_flight: solver.max_in_flight,
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
    endpoints: Vec<url::Url>,
    #[serde(with = "humantime_serde")]
    request_timeout: Duration,
    #[serde(with = "humantime_serde")]
    confirm_transaction_initial_timeout: Duration,
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
    max_in_flight: usize,
}

#[cfg(test)]
mod tests {
    use {super::*, std::path::Path};

    #[tokio::test]
    async fn load_example_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("example.toml");
        let config = load(&path).await;

        assert_eq!(config.rpc.endpoints.len(), 1);
        assert_eq!(
            config.rpc.confirm_transaction_initial_timeout,
            Duration::from_secs(10)
        );
        assert_eq!(config.solvers.len(), 1);
        assert_eq!(config.solvers[0].name, "baseline");
        assert_eq!(config.solvers[0].max_in_flight, 1);
    }

    #[tokio::test]
    #[should_panic(expected = "at least one RPC endpoint")]
    async fn missing_endpoints() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
            [chain]
            settlement-program-id = "11111111111111111111111111111111"

            [rpc]
            endpoints = []
            request-timeout = "10s"
            confirm-transaction-initial-timeout = "10s"

            [http]
            bind-address = "0.0.0.0:8080"

            [[solvers]]
            name = "baseline"
            endpoint = "http://localhost:8001"
            max-in-flight = 1
            "#,
        )
        .unwrap();
        load(&path).await;
    }
}
