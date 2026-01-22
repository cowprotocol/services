use {
    bigdecimal::Zero,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{CancellationPayload, OrderCancellation, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::ethrpc::Web3,
    std::collections::HashSet,
};

#[tokio::test]
#[ignore]
async fn local_node_single_order_cancellation() {
    run_test(single_order_cancellation).await;
}

#[tokio::test]
#[ignore]
async fn local_node_solver_competition_v1_endpoints() {
    run_test(solver_competition_v1_endpoints).await;
}

/// Test the deprecated single order cancellation endpoint (DELETE
/// /api/v1/orders/{UID})
async fn single_order_cancellation(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 10u64.eth()).await;

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Create an order
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
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

    // Verify order exists
    let order_result = services.get_order(&uid).await;
    assert!(
        order_result.is_ok(),
        "Order should exist before cancellation"
    );

    // Cancel the order using the deprecated single order cancellation endpoint
    let cancellation =
        OrderCancellation::for_order(uid, &onchain.contracts().domain_separator, &trader.signer);

    let payload = CancellationPayload {
        signature: cancellation.signature,
        signing_scheme: cancellation.signing_scheme,
    };

    let result = services.cancel_order_single(&uid, &payload).await;

    assert_eq!(
        result.unwrap(),
        "Cancelled",
        "Cancellation response should be 'Cancelled'"
    );

    // Verify order status is cancelled
    let status = services.get_order_status(&uid).await.unwrap();
    assert!(
        matches!(status, orderbook::dto::order::Status::Cancelled),
        "Order should be in Cancelled status"
    );
}

/// Test all deprecated v1 solver competition endpoints (by auction ID, by tx
/// hash, and latest)
async fn solver_competition_v1_endpoints(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 10u64.eth()).await;

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Create and execute an order
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
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

    // Wait for order to be settled
    onchain.mint_block().await;
    let settlement_finished = || async {
        let order = services.get_order(&uid).await.unwrap();
        !order.metadata.executed_buy_amount.is_zero()
    };
    wait_for_condition(TIMEOUT, settlement_finished)
        .await
        .unwrap();

    let indexed = || async {
        onchain.mint_block().await;
        if let Ok(trades) = services.get_trades(&uid).await {
            // there's only one trade anyway
            trades.into_iter().any(|trade| trade.tx_hash.is_some())
        } else {
            false
        }
    };
    wait_for_condition(TIMEOUT, indexed).await.unwrap();

    // Get latest competition to extract auction ID
    let latest_competition = services.get_latest_solver_competition_v1().await.unwrap();
    let auction_id = latest_competition.auction_id;
    // Test v1 endpoint: latest
    assert!(
        !latest_competition.common.solutions.is_empty(),
        "Latest competition should have at least one solution"
    );

    // Test v1 endpoint: by auction ID
    let competition_by_id = services
        .get_solver_competition_v1(auction_id)
        .await
        .unwrap();
    assert_eq!(
        competition_by_id.auction_id, auction_id,
        "Auction ID should match"
    );
    assert!(
        !competition_by_id.common.solutions.is_empty(),
        "Competition by ID should have at least one solution"
    );

    // Get trade to extract transaction hash
    let trades = services.get_trades(&uid).await.unwrap();
    // we checked that trades[0] exists in the wait_for_condition
    let tx_hash = trades[0].tx_hash.expect("Trade should have tx_hash");

    // Test v1 endpoint: by transaction hash
    let competition_by_tx = services
        .get_solver_competition_by_tx_v1(tx_hash)
        .await
        .unwrap();
    assert!(
        !competition_by_tx.common.solutions.is_empty(),
        "Competition by tx hash should have at least one solution"
    );

    // Verify consistency: all endpoints should return data about the same
    // competition
    let mut auction_ids = HashSet::<i64>::new();
    auction_ids.extend(&[
        competition_by_id.auction_id,
        competition_by_id.auction_id,
        latest_competition.auction_id,
    ]);
    assert_eq!(
        auction_ids.len(),
        1,
        "Auction IDs do not match between endpoints"
    );
}
