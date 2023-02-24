use {
    crate::{
        deploy::Contracts,
        onchain_components::{deploy_token_with_weth_uniswap_pool, to_wei, WethPoolConfig},
        services::{wait_for_condition, API_HOST},
    },
    ethcontract::{transaction::TransactionBuilder, Account, PrivateKey, H160, H256, U256},
    hex_literal::hex,
    model::{
        order::{OrderBuilder, OrderKind},
        signature::EcdsaSigningScheme,
    },
    reqwest::Url,
    secp256k1::SecretKey,
    shared::{ethrpc::Web3, sources::uniswap_v2::UNISWAP_INIT},
    std::{io::Write, time::Duration},
    tokio::task::JoinHandle,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    crate::local_node::test(test).await;
}

async fn test(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;

    tracing::info!("Setting up chain state.");
    let contracts = crate::deploy::deploy(&web3).await.unwrap();
    const SOLVER_PRIVATE_KEY: [u8; 32] =
        hex!("0000000000000000000000000000000000000000000000000000000000000001");
    let solver = Account::Offline(PrivateKey::from_raw(SOLVER_PRIVATE_KEY).unwrap(), None);
    contracts
        .gp_authenticator
        .add_solver(solver.address())
        .send()
        .await
        .unwrap();
    const TRADER_A_PK: [u8; 32] =
        hex!("0000000000000000000000000000000000000000000000000000000000000002");
    let trader = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    for address in [&solver.address(), &trader.address()] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(10))
            .to(*address)
            .send()
            .await
            .unwrap();
    }
    let token = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    let token = token.contract;
    tx!(
        trader,
        contracts.weth.approve(contracts.allowance, to_wei(3))
    );
    tx_value!(trader, to_wei(3), contracts.weth.deposit());

    tracing::info!("Starting services.");
    let (solver_endpoint, _) = start_solver(contracts.weth.address()).await;
    start_driver(
        &contracts,
        &solver_endpoint,
        &solver.address(),
        &SOLVER_PRIVATE_KEY,
    );
    let driver_url: Url = "http://localhost:11088/test_solver".parse().unwrap();
    let autopilot_args = &[
        "--enable-colocation".to_string(),
        format!("--drivers={driver_url}"),
        format!(
            "--trusted-tokens=0x{},0x{}",
            hex::encode(token.address()),
            hex::encode(contracts.weth.address())
        ),
    ];
    crate::services::start_autopilot(&contracts, autopilot_args);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    tracing::info!("Placing order");
    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let order_a = OrderBuilder::default()
        .with_sell_token(contracts.weth.address())
        .with_sell_amount(to_wei(2))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token.address())
        .with_buy_amount(to_wei(1))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Buy)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_A_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = http
        .post(&format!("{API_HOST}/api/v1/orders"))
        .json(&order_a)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);

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
    let config = format!(
        r#"
weth = "{weth:?}"
base-tokens = []
max-hops = 0
        "#,
    );
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();
    let file = file.into_temp_path();
    let args = vec![
        "solvers".to_string(),
        "baseline".to_string(),
        format!("--config={}", file.to_str().unwrap()),
    ];
    let (bind, bind_receiver) = tokio::sync::oneshot::channel();
    let task = async move {
        let _file = file;
        solvers::run::run(args.into_iter(), Some(bind)).await;
    };
    let handle = tokio::task::spawn(task);
    let solver_addr = bind_receiver.await.unwrap();
    let url = format!("http://{solver_addr}").parse().unwrap();
    (url, handle)
}

fn start_driver(
    contracts: &Contracts,
    solver_endpoint: &Url,
    solver_account_address: &H160,
    solver_account_private_key: &[u8; 32],
) -> JoinHandle<()> {
    let config = format!(
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
address = "{solver_account_address:?}"
private-key = "0x{}"

[liquidity]
base-tokens = []

[[liquidity.uniswap-v2]]
router = "{:?}"
pool-code = "{:?}"

[submission]
gas-price-cap = 1000000000000

[[submission.mempool]]
mempool = "public"
"#,
        contracts.gp_settlement.address(),
        contracts.weth.address(),
        hex::encode(solver_account_private_key),
        contracts.uniswap_router.address(),
        H256(UNISWAP_INIT),
    );
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();
    let file = file.into_temp_path();
    let args = vec![
        "driver".to_string(),
        format!("--config={}", dbg!(file.to_str().unwrap())),
        "--ethrpc=http://localhost:8545".to_string(),
    ];
    let task = async move {
        let _file = file;
        driver::run::run(args.into_iter(), driver::infra::time::Now::Real, None).await;
    };
    tokio::task::spawn(task)
}
