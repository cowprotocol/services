use {
    ::alloy::primitives::U256,
    contracts::alloy::GPv2Settlement,
    database::order_events::{OrderEvent, OrderEventLabel},
    e2e::setup::{eth, *},
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
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

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance.into_alloy(), eth(3))
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address().into_alloy())
        .value(eth(3))
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    tracing::info!("Placing order");
    let balance = token
        .balanceOf(trader.address().into_alloy())
        .call()
        .await
        .unwrap();
    assert_eq!(balance, U256::ZERO);
    let order = OrderCreation {
        sell_token: onchain.contracts().weth.address().into_legacy(),
        sell_amount: to_wei(2),
        buy_token: token.address().into_legacy(),
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
    onchain
        .contracts()
        .gp_settlement
        .settle(
            Default::default(),
            Default::default(),
            Default::default(),
            [
                vec![GPv2Settlement::GPv2Interaction::Data {
                    target: trader.address().into_alloy(),
                    value: U256::ZERO,
                    callData: Default::default(),
                }],
                Default::default(),
                Default::default(),
            ],
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Waiting for trade.");
    let trade_happened = || async {
        token
            .balanceOf(trader.address().into_alloy())
            .call()
            .await
            .unwrap()
            != U256::ZERO
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let balance = token
        .balanceOf(trader.address().into_alloy())
        .call()
        .await
        .unwrap();
    assert_eq!(balance, eth(1));

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

    let data_updated = || async {
        let mut db = services.db().acquire().await.unwrap();
        let Some(auction_id) = crate::database::latest_auction_id(&mut db).await.unwrap() else {
            return false;
        };

        let participants = crate::database::auction_participants(&mut db, auction_id)
            .await
            .unwrap();
        let prices = crate::database::auction_prices(&mut db, auction_id)
            .await
            .unwrap();
        let scores = crate::database::reference_scores(&mut db, auction_id)
            .await
            .unwrap();
        // sell and buy token price can be found
        prices.iter().any(|p| p.token.0 == onchain.contracts().weth.address().0)
            && prices.iter().any(|p| p.token.0 == token.address().0)
            // solver participated in the competition
            && participants.iter().any(|p| p.0 == solver.address().0)
            // and won the auction
            && scores.first().is_some_and(|score| score.solver.0 == solver.address().0)
    };
    wait_for_condition(TIMEOUT, data_updated).await.unwrap();
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
