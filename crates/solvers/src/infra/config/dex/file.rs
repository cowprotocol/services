//! Configuration parameters that get shared across all dex solvers.

use {
    crate::{
        domain::{dex::slippage, eth, Risk},
        infra::config::unwrap_or_log,
        util::serialize,
    },
    bigdecimal::BigDecimal,
    serde::{de::DeserializeOwned, Deserialize},
    serde_with::serde_as,
    std::{fmt::Debug, num::NonZeroUsize, path::Path},
    tokio::fs,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The relative slippage allowed by the solver.
    #[serde(default = "default_relative_slippage")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_slippage: BigDecimal,

    /// The absolute slippage allowed by the solver.
    #[serde_as(as = "Option<serialize::U256>")]
    absolute_slippage: Option<eth::U256>,

    /// The number of concurrent requests to make to the DEX aggregator API.
    #[serde(default = "default_concurrent_requests")]
    concurrent_requests: NonZeroUsize,

    /// The amount of Ether a partially fillable order should be filled for at
    /// least.
    #[serde(default = "default_smallest_partial_fill")]
    #[serde_as(as = "serialize::U256")]
    smallest_partial_fill: eth::U256,

    /// Parameters used to calculate the revert risk of a solution.
    /// (gas_amount_factor, gas_price_factor, nmb_orders_factor, intercept)
    risk_parameters: (f64, f64, f64, f64),

    /// Settings specific to the wrapped dex API.
    dex: toml::Value,
}

fn default_relative_slippage() -> BigDecimal {
    BigDecimal::new(1.into(), 2) // 1%
}

fn default_concurrent_requests() -> NonZeroUsize {
    NonZeroUsize::new(1).unwrap()
}

fn default_smallest_partial_fill() -> eth::U256 {
    eth::U256::exp10(16) // 0.01 ETH
}

/// Loads the base solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load<T: DeserializeOwned>(path: &Path) -> (super::Config, T) {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));

    // Not printing detailed error because it could potentially leak secrets.
    let config = unwrap_or_log(toml::de::from_str::<Config>(&data), &path);

    let dex: T = unwrap_or_log(config.dex.try_into(), &path);

    let config = super::Config {
        slippage: slippage::Limits::new(
            config.relative_slippage,
            config.absolute_slippage.map(eth::Ether),
        )
        .expect("invalid slippage limits"),
        concurrent_requests: config.concurrent_requests,
        smallest_partial_fill: eth::Ether(config.smallest_partial_fill),
        risk: Risk {
            gas_amount_factor: config.risk_parameters.0,
            gas_price_factor: config.risk_parameters.1,
            nmb_orders_factor: config.risk_parameters.2,
            intercept: config.risk_parameters.3,
        },
    };
    (config, dex)
}
