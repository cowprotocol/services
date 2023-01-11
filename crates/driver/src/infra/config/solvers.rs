use {
    crate::{
        domain::eth,
        infra::{self, solver},
        util::serialize,
    },
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

/// Load the solver configuration from a TOML file. Panics if the config is
/// invalid or on I/O errors.
pub async fn load(path: &Path, now: infra::time::Now) -> Vec<solver::Config> {
    let data = fs::read(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    let config: Config = toml::de::from_slice(&data)
        .unwrap_or_else(|e| panic!("TOML syntax error while reading {path:?}: {e:?}"));
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
            now,
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
