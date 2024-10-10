use {
    database::order_events::{OrderEvent, OrderEventLabel},
    e2e::{setup::*, tx, tx_value},
    ethcontract::U256,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader_a.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader_a.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );
    tx!(
        trader_b.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader_b.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    tracing::info!("Placing order");
    let order_a = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: to_wei(2),
        buy_token: token.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let order_b = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: to_wei(2),
        buy_token: token.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    let uid_a = services.create_order(&order_a).await.unwrap();
    let uid_b = services.create_order(&order_b).await.unwrap();

    tracing::info!("Withdrawing WETH to render the order invalid due to insufficient funds");
    tx!(
        trader_a.account(),
        onchain.contracts().weth.withdraw(to_wei(3))
    );
    tx!(
        trader_b.account(),
        onchain.contracts().weth.withdraw(to_wei(3))
    );

    let orders_are_invalid = || async {
        let events_a = crate::database::events_of_order(services.db(), &uid_a).await;
        let events_b = crate::database::events_of_order(services.db(), &uid_b).await;
        let order_a_correct_events = events_a.into_iter().map(|e| e.label).collect::<Vec<_>>()
            == vec![OrderEventLabel::Created, OrderEventLabel::Invalid];
        let order_b_correct_events = events_b.into_iter().map(|e| e.label).collect::<Vec<_>>()
            == vec![OrderEventLabel::Created, OrderEventLabel::Invalid];
        order_a_correct_events && order_b_correct_events
    };
    wait_for_condition(TIMEOUT, orders_are_invalid)
        .await
        .unwrap();

    // Make sure that the next update is happened and no new Invalid event is
    // received for the `order_b`. `order_a` is required to track if the next update
    // is happened.
    tx_value!(
        trader_a.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );
    onchain.mint_block().await;
    let orders_updated = || async {
        let events_a = crate::database::events_of_order(services.db(), &uid_a).await;
        let events_b = crate::database::events_of_order(services.db(), &uid_b).await;
        let order_b_correct_events = events_b.into_iter().map(|e| e.label).collect::<Vec<_>>()
            == vec![OrderEventLabel::Created, OrderEventLabel::Invalid];
        events_a.len() > 2
            && check_non_consecutive_invalid_events(&events_a)
            && order_b_correct_events
    };
    wait_for_condition(TIMEOUT, orders_updated).await.unwrap();

    // Another update should proceed with the order_b.
    tx_value!(
        trader_b.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );
    onchain.mint_block().await;
    let orders_updated = || async {
        let events_a = crate::database::events_of_order(services.db(), &uid_b).await;
        let events_b = crate::database::events_of_order(services.db(), &uid_b).await;
        events_a.last().map(|o| o.label) == Some(OrderEventLabel::Traded)
            && events_b.last().map(|o| o.label) == Some(OrderEventLabel::Traded)
            && check_non_consecutive_invalid_events(&events_a)
            && check_non_consecutive_invalid_events(&events_b)
    };
    wait_for_condition(TIMEOUT, orders_updated).await.unwrap();
}

fn check_non_consecutive_invalid_events(events: &[OrderEvent]) -> bool {
    !events
        .windows(2)
        .map(|w| (w[0].label, w[1].label))
        .any(|window| window == (OrderEventLabel::Invalid, OrderEventLabel::Invalid))
}
