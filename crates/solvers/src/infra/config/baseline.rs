use {
    crate::{
        domain::{eth, solver},
        infra::{contracts, tx_gas},
    },
    balance_overrides::BalanceOverrides,
    chain::Chain,
    price_estimation::gas::SETTLEMENT_OVERHEAD,
    reqwest::Url,
    serde::Deserialize,
    simulator::swap_simulator::SwapSimulator,
    std::{path::Path, sync::Arc},
    tokio::fs,
};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// Optional chain ID. This is used to automatically determine the address
    /// of the WETH contract.
    chain_id: Option<Chain>,

    /// Optional WETH contract address. This can be used to specify a manual
    /// value **instead** of using the canonical WETH contract for the
    /// configured chain.
    weth: Option<eth::Address>,

    /// List of base tokens to use when path finding. This defines the tokens
    /// that can appear as intermediate "hops" within a trading route. Note that
    /// WETH is always considered as a base token.
    base_tokens: Vec<eth::Address>,

    /// The maximum number of hops to consider when finding the optimal trading
    /// path.
    max_hops: usize,

    /// The maximum number of pieces to divide partially fillable limit orders
    /// when trying to solve it against baseline liquidity.
    max_partial_attempts: usize,

    /// Units of gas that get added to the gas estimate for executing a
    /// computed trade route to arrive at a gas estimate for a whole settlement.
    #[serde(default = "default_gas_offset")]
    solution_gas_offset: i64,

    /// The amount of the native token to use to estimate native price of a
    /// token
    native_token_price_estimation_amount: eth::U256,

    /// If this is configured the solver will also use the Uniswap V3 liquidity
    /// sources that rely on RPC request.
    uni_v3_node_url: Option<Url>,

    /// If set, the solver will simulate each solution's full settlement
    /// transaction to obtain an accurate gas estimate that includes order
    /// hook costs. Requires `chain-id` to be set.
    gas_simulation_node_url: Option<Url>,

    /// Explicit settlement contract address for gas simulation. When provided,
    /// `chain-id` is not required for gas simulation. Useful for local test
    /// environments where contracts are deployed at non-canonical addresses.
    gas_simulation_settlement: Option<eth::Address>,
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> solver::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    // Not printing detailed error because it could potentially leak secrets.
    let config = super::unwrap_or_log(toml::de::from_str::<Config>(&data), &path);
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

    let tx_gas_estimator = if let Some(url) = config.gas_simulation_node_url {
        let settlement_addr = if let Some(addr) = config.gas_simulation_settlement {
            addr
        } else {
            let chain_id = config.chain_id.expect(
                "invalid configuration: `chain-id` is required when `gas-simulation-node-url` \
                 is set and `gas-simulation-settlement` is not provided",
            );
            contracts::Contracts::for_chain(chain_id).settlement
        };
        let web3 = ethrpc::web3(Default::default(), &url, Some("tx-gas"));
        #[allow(deprecated)]
        let current_block =
            ethrpc::block_stream::current_block_stream(url.clone(), Default::default())
                .await
                .expect("failed to create block stream for tx gas estimator");
        let balance_overrides = Arc::new(BalanceOverrides::new(web3.clone()));
        let swap_simulator = SwapSimulator::new(
            balance_overrides,
            settlement_addr,
            weth.0,
            current_block,
            web3,
            15_000_000u64,
        )
        .await
        .expect("failed to create swap simulator for tx gas estimator");
        Some(Arc::new(tx_gas::TxGasEstimator::new(swap_simulator)))
    } else {
        None
    };

    solver::Config {
        weth,
        base_tokens: config
            .base_tokens
            .into_iter()
            .map(eth::TokenAddress)
            .collect(),
        max_hops: config.max_hops,
        max_partial_attempts: config.max_partial_attempts,
        solution_gas_offset: config.solution_gas_offset.into(),
        native_token_price_estimation_amount: config.native_token_price_estimation_amount,
        uni_v3_node_url: config.uni_v3_node_url,
        tx_gas_estimator,
    }
}

/// Returns minimum gas used for settling a single order.
/// (not accounting for the cost of additional interactions)
fn default_gas_offset() -> i64 {
    SETTLEMENT_OVERHEAD.try_into().unwrap()
}
