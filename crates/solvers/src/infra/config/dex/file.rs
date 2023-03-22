//! Configuration parameters that get shared across all dex solvers.

use {
    crate::{
        domain::{dex::slippage, eth},
        infra::contracts,
        util::conv,
    },
    bigdecimal::BigDecimal,
    ethereum_types::H160,
    serde::{de::DeserializeOwned, Deserialize},
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The relative slippage allowed by the solver.
    #[serde(default = "default_relative_slippage")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_slippage: BigDecimal,

    /// The absolute slippage allowed by the solver.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    absolute_slippage: Option<BigDecimal>,

    /// The amount of eth a partially fillable order should be filled for at
    /// least.
    #[serde(default = "default_smallest_partial_fill")]
    smallest_partial_fill: eth::U256,
}

fn default_endpoint() -> reqwest::Url {
    "https://api.0x.org/swap/v1/".parse().unwrap()
}

fn default_affiliate() -> H160 {
    contracts::Contracts::for_chain(eth::ChainId::Mainnet)
        .settlement
        .0
}

fn default_relative_slippage() -> BigDecimal {
    BigDecimal::new(1.into(), 2) // 1%
}

fn default_smallest_partial_fill() -> eth::U256 {
    eth::U256::exp10(16) // 0.01 ETH
}

/// Load the base solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn parse<T: DeserializeOwned>(path: &Path) -> T {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));

    toml::de::from_str::<T>(&data)
        .unwrap_or_else(|_| panic!("TOML syntax error while reading {path:?}"))
}

pub async fn load_base_config(path: &Path) -> super::BaseConfig {
    let config: Config = parse(path).await;

    super::BaseConfig {
        slippage: slippage::Limits::new(
            config.relative_slippage,
            config.absolute_slippage.map(|value| {
                conv::decimal_to_ether(&value).expect("invalid absolute slippage Ether value")
            }),
        )
        .expect("invalid slippage limits"),
        smallest_partial_fill: eth::Ether(config.smallest_partial_fill),
    }
}
