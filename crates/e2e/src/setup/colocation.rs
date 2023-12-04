use {
    crate::{nodes::NODE_HOST, setup::*},
    ethcontract::{H160, H256},
    reqwest::Url,
    shared::sources::uniswap_v2::UNISWAP_INIT,
    tokio::task::JoinHandle,
};

pub async fn start_solver(weth: H160) -> Url {
    let config_file = config_tmp_file(format!(
        r#"
weth = "{weth:?}"
base-tokens = []
max-hops = 1
max-partial-attempts = 5
risk-parameters = [0,0,0,0]
        "#,
    ));
    let args = vec![
        "solvers".to_string(),
        "baseline".to_string(),
        format!("--config={}", config_file.display()),
    ];

    let (bind, bind_receiver) = tokio::sync::oneshot::channel();
    tokio::task::spawn(async move {
        let _config_file = config_file;
        solvers::run(args.into_iter(), Some(bind)).await;
    });

    let solver_addr = bind_receiver.await.unwrap();
    format!("http://{solver_addr}").parse().unwrap()
}

pub fn start_driver(
    contracts: &Contracts,
    solver_endpoint: &Url,
    solver_account: &TestAccount,
) -> JoinHandle<()> {
    let config_file = config_tmp_file(format!(
        r#"
[contracts]
gp-v2-settlement = "{:?}"
weth = "{:?}"

[[solver]]
name = "test_solver"
endpoint = "{solver_endpoint}"
relative-slippage = "0.1"
account = "0x{}"

[liquidity]
base-tokens = []

[[liquidity.uniswap-v2]]
router = "{:?}"
pool-code = "{:?}"
missing-pool-cache-time-seconds = 3600

[submission]
gas-price-cap = 1000000000000

[[submission.mempool]]
mempool = "public"
"#,
        contracts.gp_settlement.address(),
        contracts.weth.address(),
        hex::encode(solver_account.private_key()),
        contracts.uniswap_v2_router.address(),
        H256(UNISWAP_INIT),
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
