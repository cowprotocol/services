use {
    crate::{
        domain::eth,
        infra::{config::dex::file, dex::paraswap},
    },
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The base URL for the ParaSwap API.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    pub endpoint: Option<reqwest::Url>,

    /// The DEXs to exclude when using ParaSwap.
    #[serde(default)]
    pub exclude_dexs: Vec<String>,

    /// The solver address.
    pub address: eth::H160,

    /// Optional partner name.
    pub partner: Option<String>,
}

/// Load the ParaSwap solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    super::Config {
        paraswap: paraswap::Config {
            endpoint: config
                .endpoint
                .unwrap_or_else(|| paraswap::DEFAULT_URL.parse().unwrap()),
            exclude_dexs: config.exclude_dexs,
            address: config.address,
            partner: config.partner,
        },
        base,
    }
}
