use {
    crate::setup::*,
    ethcontract::{common::DeploymentInformation, H160},
    reqwest::Url,
    std::collections::HashSet,
    tokio::task::JoinHandle,
};

pub struct SolverEngine {
    pub name: String,
    pub endpoint: Url,
    pub account: TestAccount,
    pub base_tokens: Vec<H160>,
    pub merge_solutions: bool,
}

pub async fn start_baseline_solver(
    name: String,
    account: TestAccount,
    weth: H160,
    base_tokens: Vec<H160>,
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
        }
    }
}

pub fn start_driver(
    contracts: &Contracts,
    solvers: Vec<SolverEngine>,
    liquidity: LiquidityProvider,
    quote_using_limit_orders: bool,
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
                let account = hex::encode(account.private_key());
                format!(
                    r#"
[[solver]]
name = "{name}"
endpoint = "{endpoint}"
relative-slippage = "0.1"
account = "{account}"
merge-solutions = {merge_solutions}
quote-using-limit-orders = {quote_using_limit_orders}
"#
                )
            },
        )
        .collect::<Vec<String>>()
        .join("\n");
    let liquidity = liquidity.to_string(contracts);

    let encoded_base_tokens = encode_base_tokens(base_tokens.clone());

    let cow_amms = contracts
        .cow_amm_helper
        .iter()
        .map(|contract| {
            let Some(DeploymentInformation::BlockNumber(block)) = contract.deployment_information()
            else {
                panic!("unknown deployment block for cow amm contract");
            };

            format!(
                r#"
[[contracts.cow-amms]]
index-start = {}
helper = "{:?}"
factory = "{:?}"
"#,
                block - 1, // start indexing 1 block before the contract was deployed
                contract.address(),
                contract.address(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let config_file = config_tmp_file(format!(
        r#"
[contracts]
gp-v2-settlement = "{:?}"
weth = "{:?}"
{cow_amms}

{solvers}

[liquidity]
base-tokens = [{encoded_base_tokens}]

{liquidity}

[submission]
gas-price-cap = "1000000000000"

[[submission.mempool]]
mempool = "public"
"#,
        contracts.gp_settlement.address(),
        contracts.weth.address(),
    ));
    let args = vec![
        "driver".to_string(),
        format!("--config={}", config_file.display()),
        format!("--ethrpc={NODE_HOST}"),
    ];

    tokio::task::spawn(async move {
        let _config_file = config_file;
        driver::run(args.into_iter(), None).await;
    })
}

fn encode_base_tokens(tokens: impl IntoIterator<Item = H160>) -> String {
    tokens
        .into_iter()
        .map(|token| format!(r#""{:x}""#, token))
        .collect::<Vec<_>>()
        .join(",")
}
