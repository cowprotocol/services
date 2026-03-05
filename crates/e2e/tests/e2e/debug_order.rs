use {
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    orderbook::api::DebugOrderResponse,
    reqwest::StatusCode,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_debug_order() {
    run_test(debug_order).await;
}

async fn debug_order(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

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

    let ob_config = orderbook::config::Configuration {
        debug_route_auth_tokens: [("test".to_string(), "test-secret".to_string())]
            .into_iter()
            .collect(),
        ..orderbook::config::Configuration::test_default()
    };

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs::default(),
            autopilot::config::Configuration::test("test_solver", solver.address()),
            ob_config,
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
    onchain.mint_block().await;

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        onchain.mint_block().await;
        !token
            .balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let client = services.client();

    // Helper to fetch the debug report.
    let fetch_debug_report = || async {
        let response = client
            .get(format!("{API_HOST}/api/v1/debug/order/{uid}"))
            .header("x-auth-token", "test-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.json::<DebugOrderResponse>().await.unwrap()
    };

    // Wait until the debug report is fully populated (settlement data is
    // written asynchronously, so we poll until trades appear).
    let report_populated = || async {
        let report = fetch_debug_report().await;
        !report.trades.is_empty()
    };
    wait_for_condition(TIMEOUT, report_populated).await.unwrap();

    // Deserializing into DebugOrderResponse validates all field names and types.
    let report = fetch_debug_report().await;

    assert_eq!(report.order_uid, uid.to_string());
    assert_eq!(report.order.data.kind, OrderKind::Buy);

    assert!(
        report.events.len() >= 2,
        "expected at least created+traded events, got {:?}",
        report.events
    );
    assert_eq!(report.events[0].label, "created");
    assert_eq!(report.events.last().unwrap().label, "traded");

    assert!(!report.trades.is_empty(), "expected at least one trade");
    assert!(
        !report.executions.is_empty(),
        "expected at least one execution"
    );
    assert!(
        !report.proposed_solutions.is_empty(),
        "expected at least one proposed solution"
    );
    assert!(!report.auctions.is_empty(), "expected at least one auction");
    assert!(
        !report.auctions[0].native_prices.is_empty(),
        "expected native prices for sell/buy tokens"
    );
    assert!(
        !report.settlement_attempts.is_empty(),
        "expected at least one settlement attempt"
    );

    // --- auth error cases ---

    // Missing auth header -> 401.
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/order/{uid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Wrong token -> 401.
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/order/{uid}"))
        .header("x-auth-token", "wrong-token")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Non-existent order -> 404.
    let fake_uid = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let response = client
        .get(format!("{API_HOST}/api/v1/debug/order/{fake_uid}"))
        .header("x-auth-token", "test-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
