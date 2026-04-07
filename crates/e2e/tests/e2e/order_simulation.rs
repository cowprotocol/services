use {
    alloy::{primitives::Address, providers::Provider},
    bigdecimal::BigDecimal,
    configs::test_util::TestDefault,
    database::{
        byte_array::ByteArray,
        events::{EventIndex, Trade, insert_trade},
    },
    e2e::setup::{API_HOST, OnchainComponents, Services, TIMEOUT, run_test, wait_for_condition},
    eth_domain_types::U256,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderCreation, OrderKind},
        signature::{EcdsaSigningScheme, Signature, SigningScheme},
    },
    number::{conversions::u256_to_big_decimal, units::EthUnit},
    orderbook::dto::{OrderSimulationResult, order::Status},
    reqwest::StatusCode,
    simulator::tenderly::dto::SimulationType,
    std::ops::DerefMut,
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

// Trader has 1 WETH; the order is a partially-fillable sell of 2 WETH.
//
// - executed_amount=0  → simulation must fail (full 2 WETH needed, only 1
//   available)
// - executed_amount=1e18 → simulation must pass (only 1 WETH remaining, which
//   is exactly what the trader holds)
// - no query param, but DB trade row records 1 WETH executed → simulation must
//   pass (reads fill state from the database)
async fn order_simulation_partial_fill(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund with 2 WETH so the order passes balance validation on submission.
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(2u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 2u64.eth())
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

    // Transfer 1 WETH away so the trader now holds only 1 WETH.
    let burn = Address::from([0x42u8; 20]);
    onchain
        .contracts()
        .weth
        .transfer(burn, 1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let client = services.client();

    // executed_amount=0: simulate the full 2 WETH — must fail because the
    // trader only holds 1 WETH.
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/simulation/{uid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert!(
        result.error.is_some(),
        "expected simulation failure with executed_amount=0 (needs 2 WETH, trader has 1)"
    );
    assert!(result.error.unwrap().contains("reverted"));

    /*
    TODO: Update the test as per partial_fill.rs to account for partial fill and make sure
    simulation still passes (as the executed amount is fetched by an RPC call)
    // Insert a fake trade into the database recording 1 WETH as already
    // executed for this order.
    let db = services.db();
    let mut conn = db.acquire().await.unwrap();
    insert_trade(
        conn.deref_mut(),
        &EventIndex {
            block_number: 1,
            log_index: 0,
        },
        &Trade {
            order_uid: ByteArray(uid.0),
            sell_amount_including_fee: u256_to_big_decimal(&1u64.eth()),
            // 0.5 ETH already filled for the order
            buy_amount: u256_to_big_decimal(&(1u64.eth() / 2u64.atom())),
            fee_amount: BigDecimal::default(),
        },
    )
    .await
    .unwrap();

    // (1 WETH executed), leaving 1 WETH to simulate.  The trader
    // holds exactly 1 WETH, so the simulation must pass.
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/simulation/{uid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = response.json::<OrderSimulationResult>().await.unwrap();
    assert_eq!(
        result.error, None,
        "expected simulation success when DB shows 1 WETH executed (remaining 1 WETH, trader has \
         1), got error: {:?}",
        result.error
    );
    */
}
