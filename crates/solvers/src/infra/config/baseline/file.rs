use {
    crate::{domain::eth, util::serialize},
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The chain ID the solver is configured for.
    #[serde_as(as = "serialize::ChainId")]
    pub chain_id: eth::ChainId,

    /// The address of the WETH contract.
    pub weth: Option<eth::H160>,

    /// List of base tokens to use when path finding. This defines the tokens
    /// that can appear as intermediate "hops" within a trading route. Note that
    /// WETH is always considered as a base token.
    pub base_tokens: Vec<eth::H160>,

    /// The maximum number of hops to consider when finding the optimal trading
    /// path.
    pub max_hops: usize,
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
    let config: Config = toml::de::from_str(&data)
        .unwrap_or_else(|e| panic!("TOML syntax error while reading {path:?}: {e:?}"));
    super::BaselineConfig {
        chain_id: config.chain_id,
        weth: config.weth.map(eth::WethAddress),
        base_tokens: config
            .base_tokens
            .into_iter()
            .map(eth::TokenAddress)
            .collect(),
        max_hops: config.max_hops,
    }
}
