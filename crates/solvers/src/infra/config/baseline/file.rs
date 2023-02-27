use {
    crate::{domain::eth, infra::contracts, util::serialize},
    ethereum_types::H160,
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// Optional chain ID. This is used to automatically determine the address
    /// of the WETH contract.
    #[serde_as(as = "Option<serialize::ChainId>")]
    chain_id: Option<eth::ChainId>,

    /// Optional WETH contract address. This can be used to specify a manual
    /// value **instead** of using the canonical WETH contract for the
    /// configured chain.
    weth: Option<H160>,

    /// List of base tokens to use when path finding. This defines the tokens
    /// that can appear as intermediate "hops" within a trading route. Note that
    /// WETH is always considered as a base token.
    base_tokens: Vec<eth::H160>,

    /// The maximum number of hops to consider when finding the optimal trading
    /// path.
    max_hops: usize,
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    let config = toml::de::from_str::<Config>(&data)
        .unwrap_or_else(|e| panic!("TOML syntax error while reading {path:?}: {e:?}"));
    let weth = match (config.chain_id, config.weth) {
        (Some(chain_id), None) => contracts::Contracts::for_chain(chain_id).weth,
        (None, Some(weth)) => eth::WethAddress(weth),
        (Some(_), Some(_)) => panic!(
            "invalid configuration: cannot specify both `chain-id` and `weth` configuration \
             options",
        ),
        (None, None) => panic!(
            "invalid configuration: must specify either `chain-id` or `weth` configuration options",
        ),
    };

    super::Config {
        weth,
        base_tokens: config
            .base_tokens
            .into_iter()
            .map(eth::TokenAddress)
            .collect(),
        max_hops: config.max_hops,
    }
}
