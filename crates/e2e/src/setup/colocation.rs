use {
    crate::{nodes::NODE_WS_HOST, setup::*},
    ::alloy::primitives::Address,
    reqwest::Url,
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
}

pub async fn start_baseline_solver(
    name: String,
    account: TestAccount,
    weth: Address,
    base_tokens: Vec<Address>,
    max_hops: usize,
    merge_solutions: bool,
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
    UniswapV3 { subgraph: Url },
    ZeroEx { api_port: u16 },
}

impl LiquidityProvider {
    pub fn to_string(&self, contracts: &Contracts) -> String {
        match self {
            Self::UniswapV2 => format!(
                r#"
[[liquidity.uniswap-v2]]
router = "{:?}"
pool-code = "{:?}"
missing-pool-cache-time = "0s"
"#,
                contracts.uniswap_v2_router.address(),
                contracts.default_pool_code()
            ),
            Self::ZeroEx { api_port } => format!(
                r#"
[liquidity.zeroex]
base-url = {:?}
api-key = {:?}
http-timeout = "10s"
"#,
                format!("http://0.0.0.0:{}", api_port),
                "no-api-key".to_string()
            ),
            Self::UniswapV3 { subgraph } => format!(
                r#"
[[liquidity.uniswap-v3]]
preset = "uniswap-v3"
graph-url = "{subgraph}"
"#
            ),
        }
    }
}

pub fn start_driver(
    contracts: &Contracts,
    solvers: Vec<SolverEngine>,
    liquidity: LiquidityProvider,
    quote_using_limit_orders: bool,
) -> JoinHandle<()> {
    start_driver_with_config_override(
        contracts,
        solvers,
        liquidity,
        quote_using_limit_orders,
        None,
    )
}

pub fn start_driver_with_config_override(
    contracts: &Contracts,
    solvers: Vec<SolverEngine>,
    liquidity: LiquidityProvider,
    quote_using_limit_orders: bool,
    config_override: Option<&str>,
) -> JoinHandle<()> {
    let base_tokens: HashSet<_> = solvers
        .iter()
        .flat_map(|solver| solver.base_tokens.iter())
        .cloned()
        .collect();
    let solvers = solvers
        .iter()
        .map(
            |SolverEngine {
                 name,
                 account,
                 endpoint,
                 base_tokens: _,
                 merge_solutions,
             }| {
                let account = account.signer.to_bytes();
                format!(
                    r#"
[[solver]]
name = "{name}"
endpoint = "{endpoint}"
relative-slippage = "0.1"
account = "{account}"
merge-solutions = {merge_solutions}
quote-using-limit-orders = {quote_using_limit_orders}
enable-simulation-bad-token-detection = true
enable-metrics-bad-token-detection = true
http-time-buffer = "100ms"
solving-share-of-deadline = 1.0
"#
                )
            },
        )
        .collect::<Vec<String>>()
        .join("\n");
    let liquidity = liquidity.to_string(contracts);

    let encoded_base_tokens = encode_base_tokens(base_tokens.clone());
    let flashloan_router_config = contracts
        .flashloan_router
        .as_ref()
        .map(|contract| format!("flashloan-router = \"{:?}\"", contract.address()))
        .unwrap_or_default();

    let base_config = format!(
        r#"
app-data-fetching-enabled = true
orderbook-url = "http://localhost:8080"
flashloans-enabled = true
tx-gas-limit = "45000000"

[gas-estimator]
estimator = "web3"

[contracts]
gp-v2-settlement = "{:?}"
weth = "{:?}"
balances = "{:?}"
signatures = "{:?}"
{flashloan_router_config}

{solvers}

[liquidity]
base-tokens = [{encoded_base_tokens}]

{liquidity}

[submission]
gas-price-cap = "1000000000000"

[[submission.mempool]]
url = "{NODE_HOST}"

[pod]
endpoint = {:?}
auction-contract-address = {:?}
"#,
        contracts.gp_settlement.address(),
        contracts.weth.address(),
        contracts.balances.address(),
        contracts.signatures.address(),
        config::pod::POD_ENDPOINT,
        config::pod::POD_AUCTION_CONTRACT,
    );

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
