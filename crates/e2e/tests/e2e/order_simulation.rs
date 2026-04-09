use {
    alloy::{primitives::Address, providers::Provider},
    configs::test_util::TestDefault,
    e2e::setup::{API_HOST, OnchainComponents, Services, TIMEOUT, run_test, wait_for_condition},
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    orderbook::dto::OrderSimulationResult,
    reqwest::StatusCode,
    simulator::tenderly::dto::SimulationType,
};

#[tokio::test]
#[ignore]
async fn local_node_order_simulation() {
    run_test(order_simulation).await;
}

#[tokio::test]
#[ignore]
async fn local_node_custom_order_simulation() {
    run_test(custom_order_simulation).await;
}

#[tokio::test]
#[ignore]
async fn local_node_order_simulation_partial_fill() {
    run_test(order_simulation_partial_fill).await;
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
    let request = orderbook::dto::OrderSimulationRequest {
        sell_token: *token.address(),
        buy_token: *onchain.contracts().weth.address(),
        sell_amount: sell_amount.try_into().expect("Sell amount is non zero"),
        buy_amount: 1u64.eth(),
        kind: OrderKind::Sell,
        owner: trader.address(),
        ..Default::default()
    };

    // Trader has no sell tokens — simulation should revert.
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/simulation"))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert!(
        result.error.is_some(),
        "expected simulation error when trader has no funds"
    );
    assert!(result.error.unwrap().contains("reverted"));

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
        .json(&request)
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
    assert!(result.error.unwrap().contains("reverted"));

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

    // Simulation at the latest block (block_number parameter omitted), must
    // succeed.
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/simulation/{uid}"))
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

    let tenderly = result.tenderly_request;
    // check if the fields that are directly derived from the simulation have
    // correct values in the tenderly request object
    assert_eq!(tenderly.to, *onchain.contracts().gp_settlement.address());
    assert_eq!(tenderly.simulation_type, Some(SimulationType::Full));
    assert_eq!(tenderly.value, None);
}

// Uses a shallow pool to force a partial fill (same setup as partial_fill.rs).
// The test verifies two things:
//
// 1. Before any on-chain fill: filledAmount=0, full 4 WETH needed; trader only
//    has 1 WETH → simulation must fail.
//
// 2. After ~2 WETH is settled on-chain (pool depth limits the fill): the
//    simulator reads filledAmount from the settlement contract and only
//    simulates the ~2 WETH remaining.  Trader holds ~2 WETH, so simulation must
//    pass.  If the simulator did NOT read on-chain state it would try to
//    simulate the full 4 WETH and revert (trader only has ~2 WETH).
async fn order_simulation_partial_fill(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    // Shallow pool forces the solver to only partially fill the order
    // (same pool size as partial_fill.rs).
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(10u64.eth(), 10u64.eth())
        .await;

    // Fund with 4 WETH so the order passes balance validation on submission.
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(4u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 4u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    // Same order as partial_fill.rs: pool can only fill ~2 WETH at this price.
    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 4u64.eth(),
        buy_token: *token.address(),
        buy_amount: 3u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: true,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    // Before any block is minted the autopilot has not yet processed the order.
    // Burn 3 WETH so the trader holds only 1 WETH; filledAmount is still 0.
    let burn = Address::from([0x42u8; 20]);
    onchain
        .contracts()
        .weth
        .transfer(burn, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let client = services.client();

    // filledAmount=0 on-chain; full 4 WETH needed; trader only has 1 → must fail.
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/simulation/{uid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert!(
        result.error.is_some(),
        "expected simulation failure: filledAmount=0 so full 4 WETH is needed, but trader has 1"
    );
    assert!(result.error.unwrap().contains("reverted"));

    // Restore 4 WETH so the solver can actually execute the partial fill.
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    // Trigger the autopilot to pick up the order and settle a partial fill.
    onchain.mint_block().await;

    let trade_happened = || async {
        !token
            .balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Simulation must pass.  After the ~2 WETH partial fill:
    //   - filledAmount ≈ 2 WETH on-chain
    //   - remaining sell ≈ 2 WETH
    //   - trader WETH balance ≈ 2 WETH  (started with 4, ~2 sold)
    // Without reading on-chain fill state the simulator would need the full
    // 4 WETH from the trader (who only holds ~2) and revert.
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/simulation/{uid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();

    assert_eq!(
        result.error, None,
        "expected simulation success after partial fill (on-chain filledAmount reduces the \
         simulated sell amount to match trader balance), got error: {:?}",
        result.error
    );
}
