use {
    crate::{nodes::NODE_WS_HOST, setup::*},
    ::alloy::primitives::Address,
    reqwest::Url,
    serde::Serialize,
    std::collections::HashSet,
    tokio::task::JoinHandle,
};

pub mod utils;

pub struct SolverEngine {
    pub name: String,
    pub endpoint: Url,
    pub account: TestAccount,
    pub base_tokens: Vec<Address>,
    pub merge_solutions: bool,
    /// Solver fee in basis points (0-10000) for conservative bidding.
    pub solver_fee_bps: u32,
    /// Absolute cap, in wei, on the AMM slippage the driver may add. `None`
    /// leaves it unset (relative slippage only).
    pub absolute_slippage: Option<u128>,
    /// Additional EOAs that can submit settlement txs on behalf of the solver
    /// via EIP-7702 delegation. When non-empty, enables parallel submission.
    pub submission_keys: Vec<TestAccount>,
}

pub async fn start_baseline_solver(
    name: String,
    account: TestAccount,
    weth: Address,
    base_tokens: Vec<Address>,
    max_hops: usize,
    merge_solutions: bool,
) -> SolverEngine {
    start_baseline_solver_with_solver_fee(
        name,
        account,
        weth,
        base_tokens,
        max_hops,
        merge_solutions,
        0,
    )
    .await
}

pub async fn start_baseline_solver_with_solver_fee(
    name: String,
    account: TestAccount,
    weth: Address,
    base_tokens: Vec<Address>,
    max_hops: usize,
    merge_solutions: bool,
    solver_fee_bps: u32,
) -> SolverEngine {
    let encoded_base_tokens = encode_base_tokens(base_tokens.clone());
    let config_file = config_tmp_file(format!(
        r#"
weth = "{weth:?}"
base-tokens = [{encoded_base_tokens}]
max-hops = {max_hops}
max-partial-attempts = 5
native-token-price-estimation-amount = "100000000000000000"
uni-v3-node-url = "http://localhost:8545"
        "#,
    ));

    let endpoint = start_solver(config_file, "baseline".to_string()).await;
    SolverEngine {
        name,
        endpoint,
        account,
        base_tokens,
        merge_solutions,
        solver_fee_bps,
        absolute_slippage: None,
        submission_keys: vec![],
    }
}

async fn start_solver(config_file: TempPath, solver_name: String) -> Url {
    let args = vec![
        "solvers".to_string(),
        "--addr=0.0.0.0:0".to_string(),
        solver_name,
        format!("--config={}", config_file.display()),
    ];

    let (bind, bind_receiver) = tokio::sync::oneshot::channel();
    tokio::task::spawn(async move {
        let _config_file = config_file;
        solvers::run(args, Some(bind)).await;
    });

    let solver_addr = bind_receiver.await.unwrap();
    format!("http://{solver_addr}").parse().unwrap()
}

pub enum LiquidityProvider {
    UniswapV2,
    ZeroEx { api_port: u16 },
}

impl LiquidityProvider {
    /// The provider-specific entries under `[liquidity]`. Exactly one side is
    /// populated; the other serializes to nothing.
    fn liquidity_sources(
        &self,
        contracts: &Contracts,
    ) -> (Vec<UniswapV2Liquidity>, Option<ZeroExLiquidity>) {
        match self {
            Self::UniswapV2 => (
                vec![UniswapV2Liquidity {
                    router: format!("{:?}", contracts.uniswap_v2_router.address()),
                    pool_code: format!("{:?}", contracts.default_pool_code()),
                    missing_pool_cache_time: "0s".to_string(),
                }],
                None,
            ),
            Self::ZeroEx { api_port } => (
                vec![],
                Some(ZeroExLiquidity {
                    base_url: format!("http://0.0.0.0:{api_port}"),
                    api_key: "no-api-key".to_string(),
                    http_timeout: "10s".to_string(),
                }),
            ),
        }
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

pub fn start_driver(
    contracts: &Contracts,
    solvers: Vec<SolverEngine>,
    liquidity: LiquidityProvider,
) -> JoinHandle<()> {
    start_driver_with_config_override(contracts, solvers, liquidity, None)
}

pub fn start_driver_with_config_override(
    contracts: &Contracts,
    solvers: Vec<SolverEngine>,
    liquidity: LiquidityProvider,
    config_override: Option<&str>,
) -> JoinHandle<()> {
    let base_tokens: Vec<String> = solvers
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
    let (uniswap_v2, zeroex) = liquidity.liquidity_sources(contracts);

    let config = DriverConfig {
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
    };
    let base_config = toml::to_string_pretty(&config).expect("failed to serialize driver config");

    let final_config = if let Some(override_str) = config_override {
        utils::toml::merge_raw(&base_config, override_str).expect("Failed to merge driver config")
    } else {
        base_config
    };

    let config_file = config_tmp_file(final_config);
    let args = vec![
        "driver".to_string(),
        format!("--config={}", config_file.display()),
        format!("--ethrpc={NODE_HOST}"),
        format!("--node-ws-url={NODE_WS_HOST}"),
    ];

    tokio::task::spawn(async move {
        let _config_file = config_file;
        driver::run(args.into_iter(), None).await;
    })
}

fn encode_base_tokens(tokens: impl IntoIterator<Item = Address>) -> String {
    tokens
        .into_iter()
        .map(|token| format!(r#""{token:x}""#))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Guards the hand-written `Serialize` layout: field order must keep the
    // TOML valid (scalars before tables) and the renames must produce the
    // sections the driver parses.
    #[test]
    fn driver_config_serializes_to_expected_toml() {
        let config = DriverConfig {
            app_data_fetching_enabled: true,
            orderbook_url: "http://localhost:8080".to_string(),
            flashloans_enabled: true,
            tx_gas_limit: "45000000".to_string(),
            gas_estimator: GasEstimator {
                estimator: "web3".to_string(),
            },
            contracts: ContractsConfig {
                gp_v2_settlement: "0x01".to_string(),
                weth: "0x02".to_string(),
                balances: "0x03".to_string(),
                signatures: "0x04".to_string(),
                deadline_check: "0x05".to_string(),
                flashloan_router: Some("0x06".to_string()),
            },
            solvers: vec![SolverConfig {
                name: "test_solver".to_string(),
                endpoint: "http://localhost:1/".to_string(),
                relative_slippage: "0.1".to_string(),
                absolute_slippage: Some("10000".to_string()),
                account: "0xabc".to_string(),
                merge_solutions: true,
                fast_path_enabled: true,
                enable_metrics_bad_order_detection: true,
                http_time_buffer: "25ms".to_string(),
                solving_share_of_deadline: 1.0,
                solver_fee_bps: 0,
                submission_accounts: vec!["0xdef".to_string()],
            }],
            liquidity: LiquidityConfig {
                base_tokens: vec!["0x07".to_string()],
                refresh_cooldown: "0s".to_string(),
                uniswap_v2: vec![UniswapV2Liquidity {
                    router: "0x08".to_string(),
                    pool_code: "0x09".to_string(),
                    missing_pool_cache_time: "0s".to_string(),
                }],
                zeroex: None,
            },
            submission: SubmissionConfig {
                gas_price_cap: "1000000000000".to_string(),
                public_mempool: PublicMempool {
                    urls: vec!["http://localhost:2".to_string()],
                },
            },
            balance_cache: BalanceCache {
                refresh_cooldown: "0s".to_string(),
            },
        };

        let toml = toml::to_string_pretty(&config).unwrap();

        // Reparses as valid TOML (catches field-order / ValueAfterTable
        // breakage).
        toml::from_str::<toml::Table>(&toml).unwrap();

        for expected in [
            "app-data-fetching-enabled = true",
            "[gas-estimator]",
            "[contracts]",
            "flashloan-router = \"0x06\"",
            "[[solver]]",
            "absolute-slippage = \"10000\"",
            "fast-path-enabled = true",
            "submission-accounts = [\"0xdef\"]",
            "[[liquidity.uniswap-v2]]",
            "[submission.public-mempool]",
            "[balance-cache]",
        ] {
            assert!(toml.contains(expected), "missing `{expected}` in:\n{toml}");
        }
    }
}
