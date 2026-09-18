use {
    ::alloy::primitives::U256,
    configs::{autopilot::Configuration as AutopilotConfiguration, test_util::TestDefault},
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_fast_path_flags_fall_through_when_disabled() {
    run_test(fast_path_flags_fall_through_when_disabled).await;
}

/// When the fast-path handler is disabled (`fast_path_enabled = false`),
/// the orderbook still accepts an `enableFastPath` order — but the
/// autopilot classifies it into the regular auction immediately by
/// writing `valid_from = now()`, so it settles like any other order.
///
/// This guards against a regression where turning the fast-path feature
/// off at runtime would silently start rejecting integrator orders.
async fn fast_path_flags_fall_through_when_disabled(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services with the fast-path handler disabled.");
    let services = Services::new(&onchain).await;
    // `fast_path_enabled` is false (the test config default), so the
    // fast-path handler still runs but skips the settle and writes
    // `valid_from = now()`, letting the regular auction pick the order
    // up on the very next cycle.
    services
        .start_protocol_with_args(
            AutopilotConfiguration::test("test_solver", solver.address()),
            configs::orderbook::Configuration::test_default(),
            solver.clone(),
        )
        .await;

    tracing::info!("Placing an order with enableFastPath.");
    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();
    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        // Loose limit so the market can fill it easily.
        buy_amount: U256::from(1u64),
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    // Placement must succeed — the orderbook no longer rejects
    // fast-path orders on missing runtime config.
    let uid = services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for the regular-auction settlement.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    // The autopilot's fast-path handler classified the order for the
    // regular auction by writing `valid_from = now()`, not a delayed
    // exclusivity window.
    let placed = services.get_order(&uid).await.unwrap();
    let valid_from = placed
        .metadata
        .valid_from
        .expect("autopilot handler should have written valid_from");
    assert!(
        valid_from <= model::time::now_in_epoch_seconds(),
        "fast-path fallthrough should not delay `valid_from`, got {valid_from}",
    );
}
