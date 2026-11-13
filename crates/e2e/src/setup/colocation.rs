use {
    crate::{nodes::NODE_WS_HOST, setup::*},
    ::alloy::primitives::Address,
    reqwest::Url,
    tokio::task::JoinHandle,
};

mod driver_config;
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
    let base_config = driver_config::render(contracts, &solvers, &liquidity);

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
