use {
    autopilot::config::Configuration,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    reqwest::StatusCode,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_debug_simulation() {
    run_test(debug_simulation).await;
}

async fn debug_simulation(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Trader wraps ETH and approves vault relayer.
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
        ..Default::default()
    };
    let (_ob_config_file, ob_config_arg) = ob_config.to_cli_args();

    let (_autopilot_config_file, autopilot_config_arg) =
        Configuration::test("test_solver", solver.address()).to_cli_args();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![autopilot_config_arg],
                api: vec![ob_config_arg],
            },
            solver,
        )
        .await;

    // Place an order.
    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 1u64.eth(),
        buy_token: *token.address(),
        buy_amount: 500u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    let client = services.client();

    // 1. Authenticated request should succeed.
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/order/{uid}"))
        .header("x-auth-token", "test-secret")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.get("blockNumber").is_some());
    assert!(body.get("calldata").is_some());
    assert!(body.get("from").is_some());
    assert!(body.get("callTarget").is_some());

    // 2. Request without auth header should be rejected.
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/order/{uid}"))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 3. Request with wrong token should be rejected.
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/order/{uid}"))
        .header("x-auth-token", "wrong-token")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 4. Request with empty body (no JSON) should also work.
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/order/{uid}"))
        .header("x-auth-token", "test-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 5. Non-existent order should return 404.
    let fake_uid = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let response = client
        .post(format!("{API_HOST}/api/v1/debug/order/{fake_uid}"))
        .header("x-auth-token", "test-secret")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
