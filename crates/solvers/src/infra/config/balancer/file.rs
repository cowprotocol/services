use {
    crate::{
        domain::{dex::slippage, eth},
        infra::{contracts, dex},
        util::conv,
    },
    bigdecimal::BigDecimal,
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
    /// The URL of the Balancer SOR API.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// Optional Balancer V2 Vault contract address. If not specified, the
    /// default Vault contract address will be used.
    vault: Option<H160>,

    /// Optional CoW Protocol Settlement contract address. If not specified,
    /// the default Settlement contract address will be used.
    settlement: Option<H160>,

    /// The relative slippage allowed by the solver.
    #[serde(default = "default_relative_slippage")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_slippage: BigDecimal,

    /// The absolute slippage allowed by the solver.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    absolute_slippage: Option<BigDecimal>,
}

fn default_relative_slippage() -> BigDecimal {
    BigDecimal::new(1.into(), 2) // 1%
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::BalancerConfig {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    let config = toml::de::from_str::<Config>(&data)
        .unwrap_or_else(|e| panic!("TOML syntax error while reading {path:?}: {e:?}"));

    // Balancer SOR solver only supports mainnet.
    let contracts = contracts::Contracts::for_chain(eth::ChainId::Mainnet);

    super::BalancerConfig {
        sor: dex::balancer::Config {
            endpoint: config.endpoint,
            vault: config
                .vault
                .map(eth::ContractAddress)
                .unwrap_or(contracts.balancer_vault),
            settlement: config
                .settlement
                .map(eth::ContractAddress)
                .unwrap_or(contracts.settlement),
        },
        slippage: slippage::Limits::new(
            config.relative_slippage,
            config.absolute_slippage.map(|value| {
                conv::decimal_to_ether(&value).expect("invalid absolute slippage Ether value")
            }),
        )
        .expect("invalid slippage limits"),
    }
}
