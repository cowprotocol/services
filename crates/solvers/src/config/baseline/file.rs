use {crate::domain::eth, serde::Deserialize, std::path::Path, tokio::fs};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Config {
    /// The address of the WETH contract.
    pub weth: eth::H160,

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
        weth: eth::WethAddress(config.weth),
        base_tokens: config
            .base_tokens
            .into_iter()
            .map(eth::TokenAddress)
            .collect(),
        max_hops: config.max_hops,
    }
}
