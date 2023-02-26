use {
    crate::{
        deploy::Contracts,
        helpers,
        local_node::NODE_HOST,
        onchain_components::{to_wei, OnchainComponents, TestAccount},
        services::{wait_for_condition, API_HOST},
    },
    ethcontract::{H160, H256, U256},
    model::{
        order::{OrderBuilder, OrderKind},
        signature::EcdsaSigningScheme,
    },
    reqwest::Url,
    secp256k1::SecretKey,
    shared::{ethrpc::Web3, sources::uniswap_v2::UNISWAP_INIT},
    std::time::Duration,
    tokio::task::JoinHandle,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    crate::local_node::test(test).await;
}

async fn test(web3: Web3) {
    helpers::init().await;

    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let (solver_endpoint, _) = start_solver(onchain.contracts().weth.address()).await;
    start_driver(onchain.contracts(), &solver_endpoint, &solver);
    let driver_url: Url = "http://localhost:11088/test_solver".parse().unwrap();

    let autopilot_args = &[
        "--enable-colocation".to_string(),
        format!("--drivers={driver_url}"),
    ];
    crate::services::start_autopilot(onchain.contracts(), autopilot_args);
    crate::services::start_api(onchain.contracts(), &[]);
    crate::services::wait_for_api_to_come_up().await;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    tracing::info!("Placing order");
    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let order_a = OrderBuilder::default()
        .with_sell_token(onchain.contracts().weth.address())
        .with_sell_amount(to_wei(2))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token.address())
        .with_buy_amount(to_wei(1))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Buy)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = http
        .post(&format!("{API_HOST}/api/v1/orders"))
        .json(&order_a)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201,);

    tracing::info!("Waiting for trade.");
    let trade_happened =
        || async { token.balance_of(trader.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(Duration::from_secs(10), trade_happened)
        .await
        .unwrap();

    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, to_wei(1));

    // TODO: test that we have other important per-auction data that should have
    // made its way into the DB.
}

async fn start_solver(weth: H160) -> (Url, JoinHandle<()>) {
    let config_file = helpers::config_tmp_file(format!(
        r#"
weth = "{weth:?}"
base-tokens = []
max-hops = 0
        "#,
    ));
    let args = vec![
        "solvers".to_string(),
        "baseline".to_string(),
        format!("--config={}", config_file.display()),
    ];

    let (bind, bind_receiver) = tokio::sync::oneshot::channel();
    let handle = tokio::task::spawn(async move {
        let _config_file = config_file;
        solvers::run::run(args.into_iter(), Some(bind)).await;
    });
    let solver_addr = bind_receiver.await.unwrap();
    let url = format!("http://{solver_addr}").parse().unwrap();

    (url, handle)
}

fn start_driver(
    contracts: &Contracts,
    solver_endpoint: &Url,
    solver_account: &TestAccount,
) -> JoinHandle<()> {
    let config_file = helpers::config_tmp_file(format!(
        r#"
# CI e2e tests run with hardhat, which doesn't support access lists.
disable-access-list-simulation = true

[contracts]
gp-v2-settlement = "{:?}"
weth = "{:?}"

[[solver]]
name = "test_solver"
endpoint = "{solver_endpoint}"
relative-slippage = "0.1"
address = "{:?}"
private-key = "0x{}"

[liquidity]
base-tokens = []

[[liquidity.uniswap-v2]]
router = "{:?}"
pool-code = "{:?}"

[[submission.mempool]]
mempool = "public"
"#,
        contracts.gp_settlement.address(),
        contracts.weth.address(),
        solver_account.address(),
        hex::encode(solver_account.private_key()),
        contracts.uniswap_router.address(),
        H256(UNISWAP_INIT),
    ));
    let args = vec![
        "driver".to_string(),
        format!("--config={}", config_file.display()),
        format!("--ethrpc={NODE_HOST}"),
    ];

    tokio::task::spawn(async move {
        let _config_file = config_file;
        driver::run::run(args.into_iter(), driver::infra::time::Now::Real, None).await;
    })
}
