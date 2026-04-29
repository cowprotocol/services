//! Local-node tests for the EIP-1271 creation-time simulation.
//!
//! With the orderbook configured in `Eip1271SimulationMode::Enforce`, an order
//! whose simulation reverts must be rejected at creation with HTTP 400 and an
//! `Eip1271SimulationFailed` body. A well-formed order must be accepted.

use {
    app_data::Hook,
    configs::{orderbook::Eip1271SimulationMode, test_util::TestDefault},
    e2e::setup::{MintableToken, OnchainComponents, Services, run_test, safe::Safe},
    model::order::{OrderCreation, OrderCreationAppData, OrderKind},
    number::units::EthUnit,
    reqwest::StatusCode,
    serde_json::json,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_eip1271_creation_simulation_rejects_buggy_pre_hook() {
    run_test(rejects_buggy_pre_hook).await;
}

#[tokio::test]
#[ignore]
async fn local_node_eip1271_creation_simulation_accepts_valid_order() {
    run_test(accepts_valid_order).await;
}

async fn rejects_buggy_pre_hook(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let safe = Safe::deploy(trader, web3.provider.clone()).await;

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(100_000u64.eth(), 100_000u64.eth())
        .await;
    fund_safe(&safe, &token, &onchain).await;

    let services = start_services_in_enforce_mode(&onchain, solver).await;

    // Counter has only `incrementCounter` and `setCounterToBalance`. Calling
    // it with selector 0xdeadbeef hits no dispatch and reverts.
    let counter = contracts::test::Counter::Instance::deploy(web3.provider.clone())
        .await
        .unwrap();
    let buggy_pre_hook = Hook {
        target: *counter.address(),
        call_data: vec![0xde, 0xad, 0xbe, 0xef],
        gas_limit: 100_000,
    };

    let order = sign_order_with_hooks(&safe, &onchain, &token, vec![buggy_pre_hook], vec![]);

    let err = services.create_order(&order).await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST, "body: {}", err.1);
    assert!(
        err.1.contains("Eip1271SimulationFailed"),
        "expected Eip1271SimulationFailed in body, got: {}",
        err.1
    );
}

async fn accepts_valid_order(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let safe = Safe::deploy(trader, web3.provider.clone()).await;

    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(100_000u64.eth(), 100_000u64.eth())
        .await;
    fund_safe(&safe, &token, &onchain).await;

    let services = start_services_in_enforce_mode(&onchain, solver).await;

    let order = sign_order_with_hooks(&safe, &onchain, &token, vec![], vec![]);

    let uid = services
        .create_order(&order)
        .await
        .expect("expected order to be accepted");
    let stored = services.get_order(&uid).await.unwrap();
    assert_eq!(stored.metadata.uid, uid);
}

async fn fund_safe(safe: &Safe, token: &MintableToken, onchain: &OnchainComponents) {
    token.mint(safe.address(), 10u64.eth()).await;
    safe.exec_alloy_call(
        token
            .approve(onchain.contracts().allowance, 10u64.eth())
            .into_transaction_request(),
    )
    .await;
}

async fn start_services_in_enforce_mode<'a>(
    onchain: &'a OnchainComponents,
    solver: e2e::setup::onchain_components::TestAccount,
) -> Services<'a> {
    let mut orderbook_config = configs::orderbook::Configuration::test_default();
    orderbook_config
        .order_simulation
        .as_mut()
        .expect("test_default enables order_simulation")
        .eip1271_simulation_mode = Eip1271SimulationMode::Enforce;

    let services = Services::new(onchain).await;
    services
        .start_protocol_with_args(
            configs::autopilot::Configuration::test("test_solver", solver.address()),
            orderbook_config,
            solver,
        )
        .await;
    services
}

fn sign_order_with_hooks(
    safe: &Safe,
    onchain: &OnchainComponents,
    sell_token: &MintableToken,
    pre: Vec<Hook>,
    post: Vec<Hook>,
) -> OrderCreation {
    let app_data = json!({
        "metadata": {
            "hooks": {
                "pre": pre,
                "post": post,
            },
        },
    })
    .to_string();

    let mut order = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: *sell_token.address(),
        sell_amount: 5u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        from: Some(safe.address()),
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    };
    safe.sign_order(&mut order, onchain);
    order
}
