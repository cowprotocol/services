use {
    alloy::{primitives::Address, providers::Provider},
    configs::test_util::TestDefault,
    e2e::setup::{API_HOST, OnchainComponents, Services, run_test},
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    orderbook::dto::OrderSimulationResult,
    reqwest::StatusCode,
    serde_json::json,
    simulator::tenderly::dto::SimulationType,
};

#[tokio::test]
#[ignore]
async fn local_node_order_simulation() {
    run_test(order_simulation).await;
}

#[tokio::test]
#[ignore]
async fn local_node_order_simulation_block_number() {
    run_test(order_simulation_block_number).await;
}

#[tokio::test]
#[ignore]
async fn local_node_custom_order_simulation() {
    run_test(custom_order_simulation).await;
}

async fn custom_order_simulation(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            configs::autopilot::Configuration::test("test_solver", solver.address()),
            configs::orderbook::Configuration::test_default(),
            solver,
        )
        .await;

    let client = services.client();
    let sell_amount = 1u64.eth();

    let body = json!({
        "sellToken": token.address(),
        "buyToken": onchain.contracts().weth.address(),
        "sellAmount": sell_amount.to_string(),
        "buyAmount": "1",
        "kind": "sell",
        "owner": trader.address(),
    });

    // Trader has no sell tokens — simulation should revert.
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/simulation"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert!(
        result.error.is_some(),
        "expected simulation error when trader has no funds"
    );

    // Fund the trader and approve the vault relayer.
    token.mint(trader.address(), sell_amount).await;
    token
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Simulation should now succeed.
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/simulation"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert!(
        result.error.is_none(),
        "expected simulation to pass after funding the trader"
    );
}

async fn order_simulation(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            configs::autopilot::Configuration::test("test_solver", solver.address()),
            configs::orderbook::Configuration::test_default(),
            solver,
        )
        .await;

    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 2u64.eth(),
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    let client = services.client();
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/simulation/{uid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = response.json::<OrderSimulationResult>().await.unwrap();
    assert_eq!(response.error, None);

    let tenderly = response.tenderly_request;
    // check if the fields that are directly derived from the simulation have
    // correct values in the tenderly request object
    assert_eq!(tenderly.to, *onchain.contracts().gp_settlement.address());
    assert_eq!(tenderly.simulation_type, Some(SimulationType::Full));
    assert_eq!(tenderly.value, None);
}

async fn order_simulation_block_number(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader so the order passes balance validation at submission time.
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            configs::autopilot::Configuration::test("test_solver", solver.address()),
            configs::orderbook::Configuration::test_default(),
            solver,
        )
        .await;

    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 2u64.eth(),
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    // Transfer all WETH away from the trader — now they have no sell-token
    // balance. The current block becomes the "no funds" snapshot.
    let burn = Address::from([0x42u8; 20]);
    onchain
        .contracts()
        .weth
        .transfer(burn, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    let block_no_funds = web3.provider.get_block_number().await.unwrap();

    // Re-deposit WETH. The current block now has the trader fully funded again.
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    let block_with_funds = web3.provider.get_block_number().await.unwrap();

    let client = services.client();

    // Simulation at the block where the trader had no WETH must fail.
    let response = client
        .get(format!(
            "{API_HOST}/api/v1/debug/simulation/{uid}?block_number={block_no_funds}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert!(
        result.error.is_some(),
        "expected simulation failure at block {block_no_funds} (no funds), got success"
    );

    // Simulation at the block where the trader has WETH must succeed.
    let response = client
        .get(format!(
            "{API_HOST}/api/v1/debug/simulation/{uid}?block_number={block_with_funds}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert_eq!(
        result.error, None,
        "expected simulation success at block {block_with_funds} (funded), got error: {:?}",
        result.error
    );
}
