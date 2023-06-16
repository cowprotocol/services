use {
    crate::{
        domain::{eth, solver::legacy},
        infra::contracts,
        util::serialize,
    },
    reqwest::Url,
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// Chain id used to automatically determine the address
    /// of the WETH contract and for metrics.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// The solver name used in metrics.
    solver_name: String,

    /// The URL of the endpoint that responds to solve requests.
    endpoint: String,
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> legacy::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    let config = toml::de::from_str::<Config>(&data)
        .unwrap_or_else(|_| panic!("TOML syntax error while reading {path:?}"));
    let contracts = contracts::Contracts::for_chain(config.chain_id);

    legacy::Config {
        weth: contracts.weth,
        solver_name: config.solver_name,
        chain_id: config.chain_id,
        endpoint: Url::parse(&config.endpoint).unwrap(),
    }
}
