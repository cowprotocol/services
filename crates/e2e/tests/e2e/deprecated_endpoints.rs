use {
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{CancellationPayload, OrderCancellation, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_single_order_cancellation() {
    run_test(single_order_cancellation).await;
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

    // Create an order with an unachievable `buy_amount` so no solver can fill
    // it before we get a chance to cancel: the pools only hold 1000 token_b
    // of liquidity, so demanding 10_000 token_b for 5 token_a is impossible
    // and keeps the order active until the cancellation lands.
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 10_000u64.eth(),
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

    // Verify the order is marked as invalidated. We don't check the status
    // endpoint here because that is derived from the most-recent order event
    // and the autopilot keeps emitting Ready events for every auction until
    // its solvable-orders cache picks up the cancellation — a Ready event
    // written after our Cancelled event flips the status back to Active and
    // this races with the status fetch.
    let order = services.get_order(&uid).await.unwrap();
    assert!(
        order.metadata.invalidated,
        "Order should be marked invalidated after cancellation"
    );
}
