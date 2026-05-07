//! Local-node smoke test for the EIP-1271 creation-time simulation wiring.
//!
//! A Safe-signed order with empty `app_data` is accepted, proving that
//! `OrderSimulator` runs alongside the cheap signature check without
//! disrupting the happy path. The simulation runs in shadow mode (logs
//! disagreements, never rejects). An enforce-mode rejection test will be
//! added together with the enforce-mode follow-up PR.

use {
    configs::{
        orderbook::{Configuration, OrderSimulationConfig},
        test_util::TestDefault,
    },
    e2e::setup::{MintableToken, OnchainComponents, Services, run_test, safe::Safe},
    model::order::{OrderCreation, OrderCreationAppData, OrderKind},
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_eip1271_creation_simulation_accepts_valid_order() {
    run_test(accepts_valid_order).await;
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

    let services = start_services_with_simulation(&onchain, solver).await;

    let order = sign_order(&safe, &onchain, &token);

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

async fn start_services_with_simulation<'a>(
    onchain: &'a OnchainComponents,
    solver: e2e::setup::onchain_components::TestAccount,
) -> Services<'a> {
    let orderbook_config = Configuration {
        order_simulation: Some(OrderSimulationConfig::test_default()),
        ..Configuration::test_default()
    };

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

fn sign_order(
    safe: &Safe,
    onchain: &OnchainComponents,
    sell_token: &MintableToken,
) -> OrderCreation {
    let mut order = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: *sell_token.address(),
        sell_amount: 5u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        from: Some(safe.address()),
        app_data: OrderCreationAppData::Full {
            full: "{}".to_string(),
        },
        ..Default::default()
    };
    safe.sign_order(&mut order, onchain);
    order
}
