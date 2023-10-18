use {
    crate::{
        domain::eth,
        infra::{config::dex::file, contracts, dex},
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
    /// The URL of the Balancer SOR API.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// Optional Balancer V2 Vault contract address. If not specified, the
    /// default Vault contract address will be used.
    vault: Option<H160>,
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    // Balancer SOR solver only supports mainnet.
    let contracts = contracts::Contracts::for_chain(eth::ChainId::Mainnet);

    super::Config {
        sor: dex::balancer::Config {
            endpoint: config.endpoint,
            vault: config
                .vault
                .map(eth::ContractAddress)
                .unwrap_or(contracts.balancer_vault),
            settlement: base.contracts.settlement,
        },
        base,
    }
}
