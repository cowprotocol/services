use {
    e2e::setup::*,
    ethcontract::U256,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    serde_json::json,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let cow = onchain
        .deploy_cow_weth_pool(to_wei(1_000_000), to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    cow.fund(trader.address(), to_wei(5)).await;

    // Sign a permit pre-interaction for trading.
    let permit = cow
        .permit(&trader, onchain.contracts().allowance, to_wei(5))
        .await;
    // Setup a malicious interaction for setting approvals to steal funds from
    // the settlement contract.
    let steal_cow = hook_for_transaction(
        cow.approve(trader.address(), U256::max_value())
            .from(solver.account().clone())
            .tx,
    )
    .await;
    let steal_weth = hook_for_transaction(
        onchain
            .contracts()
            .weth
            .approve(trader.address(), U256::max_value())
            .from(solver.account().clone())
            .tx,
    )
    .await;

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![]);
    services
        .start_api(vec!["--enable-custom-interactions=true".to_string()])
        .await;

    let order = OrderCreation {
        sell_token: cow.address(),
        sell_amount: to_wei(4),
        fee_amount: to_wei(1),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: json!({
                "metadata": {
                    "hooks": {
                        "pre": [permit, steal_cow],
                        "post": [steal_weth],
                    },
                },
            })
            .to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    let balance = cow.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, to_wei(5));

    tracing::info!("Waiting for trade.");
    services.start_old_driver(solver.private_key(), vec![]);
    let trade_happened = || async {
        cow.balance_of(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check matching
    let balance = onchain
        .contracts()
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    assert!(balance >= order.buy_amount);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { services.get_auction().await.auction.orders.is_empty() };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();

    // Check malicious custom interactions did not work.
    let allowance = cow
        .allowance(
            onchain.contracts().gp_settlement.address(),
            trader.address(),
        )
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::zero());
    let allowance = onchain
        .contracts()
        .weth
        .allowance(
            onchain.contracts().gp_settlement.address(),
            trader.address(),
        )
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::zero());

    // Note that the allowances were set with the `HooksTrampoline` contract!
    // This is OK since the `HooksTrampoline` contract is not used for holding
    // any funds.
    let allowance = cow
        .allowance(onchain.contracts().hooks.address(), trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::max_value());
    let allowance = onchain
        .contracts()
        .weth
        .allowance(onchain.contracts().hooks.address(), trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, U256::max_value());
}
