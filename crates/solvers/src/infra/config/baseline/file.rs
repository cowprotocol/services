use {
    crate::{domain::eth, infra::contracts, util::serialize},
    ethereum_types::H160,
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    #[serde(flatten)]
    pub contracts: ContractsConfig,

    /// List of base tokens to use when path finding. This defines the tokens
    /// that can appear as intermediate "hops" within a trading route. Note that
    /// WETH is always considered as a base token.
    pub base_tokens: Vec<eth::H160>,

    /// The maximum number of hops to consider when finding the optimal trading
    /// path.
    pub max_hops: usize,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ContractsConfig {
    ChainId(#[serde_as(as = "serialize::ChainId")] eth::ChainId),
    Weth(H160),
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::BaselineConfig {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    let config = toml::de::from_str::<Config>(&data)
        .unwrap_or_else(|e| panic!("TOML syntax error while reading {path:?}: {e:?}"));
    let contracts = match config.contracts {
        ContractsConfig::ChainId(chain_id) => contracts::Contracts::for_chain(chain_id),
        ContractsConfig::Weth(weth) => contracts::Contracts {
            weth: eth::WethAddress(weth),
        },
    };

    super::BaselineConfig {
        weth: contracts.weth,
        base_tokens: config
            .base_tokens
            .into_iter()
            .map(eth::TokenAddress)
            .collect(),
        max_hops: config.max_hops,
    }
}
