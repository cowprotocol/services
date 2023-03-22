use {
    crate::{
        domain::eth,
        infra::{config::dex::file, contracts, dex::zeroex},
    },
    ethereum_types::H160,
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The versioned URL endpoint for the 0x swap API.
    #[serde(default = "default_endpoint")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// An optional API key to use. This is needed when configuring 0x to use
    /// the gated API for partners.
    api_key: Option<String>,

    /// The list of excluded liquidity sources. Liquidity from these sources
    /// will not be considered when solving.
    #[serde(default)]
    excluded_sources: Vec<String>,

    /// The affiliate address to use. Defaults to the mainnet CoW Protocol
    /// settlement contract address.
    #[serde(default = "default_affiliate")]
    affiliate: H160,

    /// Whether or not to enable slippage protection. The slippage protection
    /// considers average negative slippage paid out in MEV when quoting,
    /// preferring private market maker orders when they are close to what you
    /// would get with on-chain liquidity pools.
    #[serde(default)]
    enable_slippage_protection: bool,
}

fn default_endpoint() -> reqwest::Url {
    "https://api.0x.org/swap/v1/".parse().unwrap()
}

fn default_affiliate() -> H160 {
    contracts::Contracts::for_chain(eth::ChainId::Mainnet)
        .settlement
        .0
}

/// Load the 0x solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    super::Config {
        zeroex: zeroex::Config {
            endpoint: config.endpoint,
            api_key: config.api_key,
            excluded_sources: config.excluded_sources,
            affiliate: config.affiliate,
            enable_slippage_protection: config.enable_slippage_protection,
        },
        base,
    }
}
