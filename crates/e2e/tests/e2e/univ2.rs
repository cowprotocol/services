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
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    tracing::info!("Placing order");
    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let order = OrderCreation {
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
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let uid = services.create_order(&order).await.unwrap();

    // Mine a trivial settlement (not encoding auction ID). This mimics fee
    // withdrawals and asserts we can handle these gracefully.
    tx!(
        solver.account(),
        onchain.contracts().gp_settlement.settle(
            Default::default(),
            Default::default(),
            Default::default(),
            [
                vec![(trader.address(), U256::from(0), Default::default())],
                Default::default(),
                Default::default()
            ],
        )
    );

    tracing::info!("Waiting for trade.");
    let trade_happened =
        || async { token.balance_of(trader.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, to_wei(1));

    let all_events_registered = || async {
        let events = crate::database::events_of_order(services.db(), &uid).await;
        order_events_matching_fuzzy(
            &events,
            &[
                OrderEventLabel::Created,
                OrderEventLabel::Ready,
                OrderEventLabel::Executing,
                OrderEventLabel::Traded,
            ],
        )
    };
    wait_for_condition(TIMEOUT, all_events_registered)
        .await
        .unwrap();

    let cip_20_data_updated = || async {
        onchain.mint_block().await;
        let data = match crate::database::most_recent_cip_20_data(services.db()).await {
            Some(data) => data,
            None => return false,
        };

        // sell and buy token price can be found
        data.prices.iter().any(|p| p.token.0 == onchain.contracts().weth.address().0)
            && data.prices.iter().any(|p| p.token.0 == token.address().0)
            // solver participated in the competition
            && data.participants.iter().any(|p| p.participant.0 == solver.address().0)
            // and won the auction
            && data.score.winner.0 == solver.address().0
    };
    wait_for_condition(TIMEOUT, cip_20_data_updated)
        .await
        .unwrap();
}

fn order_events_matching_fuzzy(actual: &[OrderEvent], expected: &[OrderEventLabel]) -> bool {
    let mut events = actual.iter();

    for expectation in expected {
        loop {
            let event = match events.next() {
                Some(event) => event,
                // we are still expecting events but none are left
                None => return false,
            };
            if event.label == *expectation {
                // found expected label; break inner loop to look for next label
                break;
            }
        }
    }
    true
}
