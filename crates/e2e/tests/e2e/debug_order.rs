use {
    configs::test_util::TestDefault,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        debug_report::DebugReport,
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
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
            .get(format!("{API_HOST}/restricted/api/v1/debug/order/{uid}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.json::<DebugReport>().await.unwrap()
    };

    // Wait until the debug report is fully populated. The different pieces
    // (trades, order events, execution data, settlement attempts) are written
    // asynchronously by different components, so we poll until all of them
    // land before asserting.
    let report_populated = || async {
        let report = fetch_debug_report().await;
        let has_terminal_event = report
            .events
            .last()
            .is_some_and(|e| e.label == "traded" || e.label == "filtered");
        let auction_populated = report.auctions.first().is_some_and(|a| {
            !a.native_prices.is_empty()
                && !a.proposed_solutions.is_empty()
                && !a.executions.is_empty()
                && !a.settlement_attempts.is_empty()
        });
        !report.trades.is_empty() && has_terminal_event && auction_populated
    };
    wait_for_condition(TIMEOUT, report_populated).await.unwrap();

    // Deserializing into DebugOrderResponse validates all field names and
    // types.
    let report = fetch_debug_report().await;

    assert_eq!(report.order_uid, uid);
    assert_eq!(report.order.data.kind, OrderKind::Buy);

    let labels: Vec<_> = report.events.iter().map(|e| e.label.as_str()).collect();
    assert!(
        labels.contains(&"created") && labels.contains(&"ready") && labels.contains(&"executing"),
        "missing expected event in {labels:?}"
    );
    // The in_flight filtered event may or may not sneak in before traded
    // depending on timing.
    let last = report.events.last().unwrap();
    assert!(
        last.label == "traded" || last.label == "filtered",
        "unexpected last event: {last:?}"
    );

    assert!(!report.trades.is_empty(), "expected at least one trade");
    assert!(!report.auctions.is_empty(), "expected at least one auction");

    let auction = &report.auctions[0];
    assert!(
        !auction.native_prices.is_empty(),
        "expected native prices for sell/buy tokens"
    );
    assert!(
        !auction.proposed_solutions.is_empty(),
        "expected at least one proposed solution"
    );
    assert!(
        !auction.executions.is_empty(),
        "expected at least one execution"
    );
    assert!(
        !auction.settlement_attempts.is_empty(),
        "expected at least one settlement attempt"
    );

    // Non-existent order -> 404.
    let fake_uid = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let response = client
        .get(format!(
            "{API_HOST}/restricted/api/v1/debug/order/{fake_uid}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore]
async fn local_node_debug_order_filter_reason() {
    run_test(debug_order_filter_reason).await;
}

async fn debug_order_filter_reason(web3: Web3) {
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

    // Withdraw WETH so the order becomes invalid due to insufficient balance.
    onchain
        .contracts()
        .weth
        .withdraw(3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let client = services.client();

    // Wait for the autopilot to pick up the order and mark it invalid.
    let has_invalid_event = || async {
        onchain.mint_block().await;
        let response = client
            .get(format!("{API_HOST}/restricted/api/v1/debug/order/{uid}"))
            .send()
            .await
            .unwrap();
        let report = response.json::<DebugReport>().await.unwrap();
        report
            .events
            .iter()
            .any(|e| e.label == "invalid" && e.reason.is_some())
    };
    wait_for_condition(TIMEOUT, has_invalid_event)
        .await
        .unwrap();

    let response = client
        .get(format!("{API_HOST}/restricted/api/v1/debug/order/{uid}"))
        .send()
        .await
        .unwrap();
    let report = response.json::<DebugReport>().await.unwrap();

    let invalid_event = report
        .events
        .iter()
        .find(|e| e.label == "invalid")
        .expect("expected an invalid event");
    assert_eq!(
        invalid_event.reason.as_deref(),
        Some("insufficient_balance"),
        "expected insufficient_balance reason, got {:?}",
        invalid_event.reason
    );

    // Non-invalid events should have no reason.
    let created = report
        .events
        .iter()
        .find(|e| e.label == "created")
        .expect("expected a created event");
    assert_eq!(created.reason, None);
}
