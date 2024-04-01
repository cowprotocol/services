use {
    crate::{
        domain::{eth, solver::baseline, Risk},
        infra::{config::unwrap_or_log, contracts},
        util::serialize,
    },
    ethereum_types::H160,
    serde::Deserialize,
    serde_with::serde_as,
    shared::price_estimation::gas::SETTLEMENT_OVERHEAD,
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

    /// The maximum number of pieces to divide partially fillable limit orders
    /// when trying to solve it against baseline liquidity.
    max_partial_attempts: usize,

    /// Parameters used to calculate the revert risk of a solution.
    /// (gas_amount_factor, gas_price_factor, nmb_orders_factor, intercept)
    risk_parameters: (f64, f64, f64, f64),

    /// Units of gas that get added to the gas estimate for executing a
    /// computed trade route to arrive at a gas estimate for a whole settlement.
    #[serde(default = "default_gas_offset")]
    solution_gas_offset: i64,
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> baseline::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    // Not printing detailed error because it could potentially leak secrets.
    let config = unwrap_or_log(toml::de::from_str::<Config>(&data), &path);
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

    baseline::Config {
        weth,
        base_tokens: config
            .base_tokens
            .into_iter()
            .map(eth::TokenAddress)
            .collect(),
        max_hops: config.max_hops,
        max_partial_attempts: config.max_partial_attempts,
        risk: Risk {
            gas_amount_factor: config.risk_parameters.0,
            gas_price_factor: config.risk_parameters.1,
            nmb_orders_factor: config.risk_parameters.2,
            intercept: config.risk_parameters.3,
        },
        solution_gas_offset: config.solution_gas_offset.into(),
    }
}

/// Returns minimum gas used for settling a single order.
/// (not accounting for the cost of additional interactions)
fn default_gas_offset() -> i64 {
    SETTLEMENT_OVERHEAD.try_into().unwrap()
}
