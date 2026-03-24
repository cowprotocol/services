//! Configuration parameters that get shared across all dex solvers.

use {
    crate::{
        domain::{
            dex::{minimum_surplus::MinimumSurplusLimits, slippage::SlippageLimits},
            eth,
        },
        infra::{blockchain, config::unwrap_or_log, contracts},
    },
    bigdecimal::{BigDecimal, Zero},
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, de::DeserializeOwned},
    serde_with::serde_as,
    std::{num::NonZeroUsize, path::Path, time::Duration},
    tokio::fs,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The node URL to use for simulations.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    node_url: reqwest::Url,

    /// Optional CoW Protocol Settlement contract address. If not specified,
    /// the default Settlement contract address will be used.
    settlement: Option<eth::Address>,

    /// The relative slippage allowed by the solver.
    #[serde(default = "default_relative_slippage")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_slippage: BigDecimal,

    /// The absolute slippage allowed by the solver.
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    absolute_slippage: Option<eth::U256>,

    /// The relative minimum surplus required by the solver.
    #[serde(default = "default_relative_minimum_surplus")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_minimum_surplus: BigDecimal,

    /// The absolute minimum surplus required by the solver.
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    absolute_minimum_surplus: Option<eth::U256>,

    /// The number of concurrent requests to make to the DEX aggregator API.
    #[serde(default = "default_concurrent_requests")]
    concurrent_requests: NonZeroUsize,

    /// The amount of Ether a partially fillable order should be filled for at
    /// least.
    #[serde(default = "default_smallest_partial_fill")]
    #[serde_as(as = "HexOrDecimalU256")]
    smallest_partial_fill: eth::U256,

    /// Back-off growth factor for rate limiting.
    #[serde(default = "default_back_off_growth_factor")]
    back_off_growth_factor: f64,

    /// Minimum back-off time in seconds for rate limiting.
    #[serde(with = "humantime_serde", default = "default_min_back_off")]
    min_back_off: Duration,

    /// Maximum back-off time in seconds for rate limiting.
    #[serde(with = "humantime_serde", default = "default_max_back_off")]
    max_back_off: Duration,

    /// Settings specific to the wrapped dex API.
    dex: toml::Value,

    /// Amount of gas that gets added to each swap to adjust the cost coverage
    /// of the solver.
    #[serde(default = "default_gas_offset")]
    #[serde_as(as = "HexOrDecimalU256")]
    gas_offset: eth::U256,

    /// How often the solver should poll the current block. If this value
    /// is set each request will also have the `X-CURRENT-BLOCK-HASH` header set
    /// updated based on the configured polling interval.
    /// This is useful for caching requests on an egress proxy.
    #[serde(with = "humantime_serde", default)]
    current_block_poll_interval: Option<Duration>,

    /// Whether to internalize the solution interactions using the Settlement
    /// contract buffers.
    #[serde(default = "default_internalize_interactions")]
    internalize_interactions: bool,
}

fn default_relative_slippage() -> BigDecimal {
    BigDecimal::new(1.into(), 2) // 1%
}

fn default_relative_minimum_surplus() -> BigDecimal {
    BigDecimal::zero() // 0%
}

fn default_concurrent_requests() -> NonZeroUsize {
    NonZeroUsize::new(1).unwrap()
}

fn default_smallest_partial_fill() -> eth::U256 {
    eth::U256::from(10).pow(eth::U256::from(16)) // 0.01 ETH
}

fn default_back_off_growth_factor() -> f64 {
    2.0
}

fn default_min_back_off() -> Duration {
    Duration::from_secs(1)
}

fn default_max_back_off() -> Duration {
    Duration::from_secs(8)
}

fn default_gas_offset() -> eth::U256 {
    // Rough estimation of the gas overhead of settling a single
    // trade via the settlement contract.
    eth::U256::from(106_391)
}

fn default_internalize_interactions() -> bool {
    true
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

    // Take advantage of the fact that deterministic deployment means that all
    // CoW Protocol contracts have the same address.
    let default_contracts = contracts::Contracts::for_chain_id(eth::ChainId::Mainnet);
    let (settlement, authenticator) = if let Some(settlement) = config.settlement {
        let authenticator =
            {
                let web3 = blockchain::rpc(&config.node_url);
                let settlement = ::contracts::alloy::GPv2Settlement::Instance::new(
                    settlement,
                    web3.provider.clone(),
                );
                settlement.authenticator().call().await.unwrap_or_else(|e| {
                    panic!("error reading authenticator contract address: {e:?}")
                })
            };
        (settlement, authenticator)
    } else {
        (
            default_contracts.settlement,
            default_contracts.authenticator,
        )
    };

    let block_stream = match config.current_block_poll_interval {
        Some(interval) => Some(
            #[allow(deprecated)]
            ethrpc::block_stream::current_block_stream(config.node_url.clone(), interval)
                .await
                .unwrap(),
        ),
        None => None,
    };

    let config = super::Config {
        node_url: config.node_url,
        contracts: super::Contracts {
            settlement,
            authenticator,
        },
        slippage: SlippageLimits::new(
            config.relative_slippage,
            config.absolute_slippage.map(eth::Ether),
        )
        .expect("invalid slippage limits"),
        minimum_surplus: MinimumSurplusLimits::new(
            config.relative_minimum_surplus,
            config.absolute_minimum_surplus.map(eth::Ether),
        )
        .expect("invalid minimum surplus limits"),
        concurrent_requests: config.concurrent_requests,
        smallest_partial_fill: eth::Ether(config.smallest_partial_fill),
        rate_limiting_strategy: configs::rate_limit::Strategy::try_new(
            config.back_off_growth_factor,
            config.min_back_off,
            config.max_back_off,
        )
        .unwrap(),
        gas_offset: eth::Gas(config.gas_offset),
        block_stream,
        internalize_interactions: config.internalize_interactions,
    };
    (config, dex)
}
