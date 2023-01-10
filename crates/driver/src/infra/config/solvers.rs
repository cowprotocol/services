use {
    crate::{domain::eth, infra::solver, util::serialize},
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

/// Load the solver configuration from a YAML file. Panics if the config is
/// invalid or on I/O errors.
pub async fn load(path: &Path) -> Vec<solver::Config> {
    let data = fs::read(path)
        .await
        .unwrap_or_else(|_| panic!("I/O error while reading {path:?}"));
    let config: Config = toml::de::from_slice(&data)
        .unwrap_or_else(|_| panic!("YAML syntax error while reading {path:?}"));
    config
        .solvers
        .into_iter()
        .map(|config| solver::Config {
            endpoint: config.endpoint,
            name: config.name.into(),
            slippage: solver::Slippage {
                relative: config.relative_slippage,
                absolute: config.absolute_slippage.map(Into::into),
            },
            address: config.address.into(),
        })
        .collect()
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    solvers: Vec<SolverConfig>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SolverConfig {
    endpoint: url::Url,
    name: String,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_slippage: bigdecimal::BigDecimal,
    #[serde_as(as = "Option<serialize::U256>")]
    absolute_slippage: Option<eth::U256>,
    address: eth::H160,
}
