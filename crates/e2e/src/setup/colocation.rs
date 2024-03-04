use {
    crate::{nodes::NODE_HOST, setup::*},
    ethcontract::{H160, U256},
    reqwest::Url,
    tokio::task::JoinHandle,
};

pub async fn start_baseline_solver(weth: H160) -> Url {
    let config_file = config_tmp_file(format!(
        r#"
weth = "{weth:?}"
base-tokens = []
max-hops = 1
max-partial-attempts = 5
risk-parameters = [0,0,0,0]
        "#,
    ));

    start_solver(config_file, "baseline".to_string()).await
}

pub async fn start_naive_solver() -> Url {
    let config_file = config_tmp_file("risk-parameters = [0,0,0,0]");
    start_solver(config_file, "naive".to_string()).await
}

pub async fn start_legacy_solver(solver_endpoint: Url, chain_id: Option<U256>) -> Url {
    let chain_id = chain_id.unwrap_or(U256::from(1));
    let config_file = config_tmp_file(format!(
        r#"
chain-id = "{chain_id}"
solver-name = "legacy"
endpoint = "{solver_endpoint}"
        "#,
    ));

    start_solver(config_file, "legacy".to_string()).await
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

pub struct SolverEngine {
    pub name: String,
    pub endpoint: Url,
    pub account: TestAccount,
}

pub fn start_driver(contracts: &Contracts, solvers: Vec<SolverEngine>) -> JoinHandle<()> {
    let solvers = solvers
        .iter()
        .map(
            |SolverEngine {
                 name,
                 account,
                 endpoint,
             }| {
                let account = hex::encode(account.private_key());
                format!(
                    r#"
[[solver]]
name = "{name}"
endpoint = "{endpoint}"
relative-slippage = "0.1"
account = "{account}"

"#
                )
            },
        )
        .collect::<Vec<String>>()
        .join("\n");
    let config_file = config_tmp_file(format!(
        r#"
[contracts]
gp-v2-settlement = "{:?}"
weth = "{:?}"

{solvers}

[liquidity]
base-tokens = []
graph-api-base-url = "https://api.thegraph.com/subgraphs/name/"

[[liquidity.uniswap-v2]]
router = "{:?}"
pool-code = "{:?}"
missing-pool-cache-time = "1h"

[submission]
gas-price-cap = "1000000000000"
logic = "native"
max-confirm-time= "2s"

[[submission.mempool]]
mempool = "public"
"#,
        contracts.gp_settlement.address(),
        contracts.weth.address(),
        contracts.uniswap_v2_router.address(),
        contracts.default_pool_code(),
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
