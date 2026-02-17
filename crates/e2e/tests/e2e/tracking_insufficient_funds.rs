use {
    database::order_events::{OrderEvent, OrderEventLabel},
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader_a.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader_b.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader_b.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    tracing::info!("Placing order");
    let order_a = OrderCreation {
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
        &trader_a.signer,
    );
    let order_b = OrderCreation {
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
        &trader_b.signer,
    );
    let uid_a = services.create_order(&order_a).await.unwrap();
    let uid_b = services.create_order(&order_b).await.unwrap();

    tracing::info!("Withdrawing WETH to render the order invalid due to insufficient funds");
    onchain
        .contracts()
        .weth
        .withdraw(3u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .withdraw(3u64.eth())
        .from(trader_b.address())
        .send_and_watch()
        .await
        .unwrap();

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
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader_a.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    let orders_updated = || async {
        onchain.mint_block().await;
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
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader_b.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    let orders_updated = || async {
        onchain.mint_block().await;
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
