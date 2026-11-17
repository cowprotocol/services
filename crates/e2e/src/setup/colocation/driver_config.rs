use {
    super::{LiquidityProvider, SolverEngine},
    crate::{nodes::NODE_HOST, setup::Contracts},
    serde::Serialize,
    std::collections::HashSet,
};

/// Serialize the driver config for a colocated test driver to a TOML string.
pub(super) fn render(
    contracts: &Contracts,
    solvers: &[SolverEngine],
    liquidity: &LiquidityProvider,
) -> String {
    toml::to_string_pretty(&build(contracts, solvers, liquidity))
        .expect("failed to serialize driver config")
}

fn build(
    contracts: &Contracts,
    solvers: &[SolverEngine],
    liquidity: &LiquidityProvider,
) -> DriverConfig {
    let base_tokens = solvers
        .iter()
        .flat_map(|solver| solver.base_tokens.iter())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|token| format!("{token:x}"))
        .collect();
    let solver_configs = solvers
        .iter()
        .map(|solver| SolverConfig {
            name: solver.name.clone(),
            endpoint: solver.endpoint.to_string(),
            relative_slippage: "0.1".to_string(),
            absolute_slippage: solver.absolute_slippage.map(|wei| wei.to_string()),
            account: solver.account.signer.to_bytes().to_string(),
            merge_solutions: solver.merge_solutions,
            fast_path_enabled: true,
            enable_metrics_bad_order_detection: true,
            http_time_buffer: "25ms".to_string(),
            solving_share_of_deadline: 1.0,
            solver_fee_bps: solver.solver_fee_bps,
            submission_accounts: solver
                .submission_keys
                .iter()
                .map(|key| key.signer.to_bytes().to_string())
                .collect(),
        })
        .collect();
    let (uniswap_v2, zeroex) = liquidity_sources(liquidity, contracts);

    DriverConfig {
        app_data_fetching_enabled: true,
        orderbook_url: "http://localhost:8080".to_string(),
        flashloans_enabled: true,
        tx_gas_limit: "45000000".to_string(),
        gas_estimator: GasEstimator {
            estimator: "web3".to_string(),
        },
        contracts: ContractsConfig {
            gp_v2_settlement: format!("{:?}", contracts.gp_settlement.address()),
            weth: format!("{:?}", contracts.weth.address()),
            balances: format!("{:?}", contracts.balances.address()),
            signatures: format!("{:?}", contracts.signatures.address()),
            deadline_check: format!("{:?}", contracts.deadline_check.address()),
            flashloan_router: contracts
                .flashloan_router
                .as_ref()
                .map(|contract| format!("{:?}", contract.address())),
        },
        solvers: solver_configs,
        liquidity: LiquidityConfig {
            base_tokens,
            refresh_cooldown: "0s".to_string(),
            uniswap_v2,
            zeroex,
        },
        submission: SubmissionConfig {
            gas_price_cap: "1000000000000".to_string(),
            public_mempool: PublicMempool {
                urls: vec![NODE_HOST.to_string()],
            },
        },
        balance_cache: BalanceCache {
            refresh_cooldown: "0s".to_string(),
        },
    }
}

/// The provider-specific entries under `[liquidity]`. Exactly one side is
/// populated; the other serializes to nothing.
fn liquidity_sources(
    liquidity: &LiquidityProvider,
    contracts: &Contracts,
) -> (Vec<UniswapV2Liquidity>, Option<ZeroExLiquidity>) {
    match liquidity {
        LiquidityProvider::UniswapV2 => (
            vec![UniswapV2Liquidity {
                router: format!("{:?}", contracts.uniswap_v2_router.address()),
                pool_code: format!("{:?}", contracts.default_pool_code()),
                missing_pool_cache_time: "0s".to_string(),
            }],
            None,
        ),
        LiquidityProvider::ZeroEx { api_port } => (
            vec![],
            Some(ZeroExLiquidity {
                base_url: format!("http://0.0.0.0:{api_port}"),
                api_key: "no-api-key".to_string(),
                http_timeout: "10s".to_string(),
            }),
        ),
    }
}

/// Typed view of the driver config file. Scalar fields are declared before
/// table fields so the serialized TOML stays valid.
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct DriverConfig {
    app_data_fetching_enabled: bool,
    orderbook_url: String,
    flashloans_enabled: bool,
    tx_gas_limit: String,
    gas_estimator: GasEstimator,
    contracts: ContractsConfig,
    #[serde(rename = "solver")]
    solvers: Vec<SolverConfig>,
    liquidity: LiquidityConfig,
    submission: SubmissionConfig,
    balance_cache: BalanceCache,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct GasEstimator {
    estimator: String,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct ContractsConfig {
    gp_v2_settlement: String,
    weth: String,
    balances: String,
    signatures: String,
    deadline_check: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    flashloan_router: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct SolverConfig {
    name: String,
    endpoint: String,
    relative_slippage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    absolute_slippage: Option<String>,
    account: String,
    merge_solutions: bool,
    fast_path_enabled: bool,
    enable_metrics_bad_order_detection: bool,
    http_time_buffer: String,
    solving_share_of_deadline: f64,
    solver_fee_bps: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    submission_accounts: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct LiquidityConfig {
    base_tokens: Vec<String>,
    refresh_cooldown: String,
    #[serde(rename = "uniswap-v2", skip_serializing_if = "Vec::is_empty")]
    uniswap_v2: Vec<UniswapV2Liquidity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zeroex: Option<ZeroExLiquidity>,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct UniswapV2Liquidity {
    router: String,
    pool_code: String,
    missing_pool_cache_time: String,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct ZeroExLiquidity {
    base_url: String,
    api_key: String,
    http_timeout: String,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct SubmissionConfig {
    gas_price_cap: String,
    public_mempool: PublicMempool,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct PublicMempool {
    urls: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct BalanceCache {
    refresh_cooldown: String,
}
