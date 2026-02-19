use {
    bigdecimal::Zero,
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
async fn local_node_trades_v2_endpoints() {
    run_test(trades_v2_endpoints).await;
}

/// Test all v2 trades endpoint features (by order UID, by owner, and
/// pagination)
async fn trades_v2_endpoints(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader1, trader2] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Mint tokens for both traders
    token_a.mint(trader1.address(), 100u64.eth()).await;
    token_a.mint(trader2.address(), 10u64.eth()).await;

    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader1.address())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader2.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Create 3 orders for trader1 (for pagination testing)
    let mut trader1_uids = Vec::new();
    for i in 0..3 {
        let order = OrderCreation {
            sell_token: *token_a.address(),
            sell_amount: (2u64 + i as u64).eth(),
            buy_token: *token_b.address(),
            buy_amount: 1u64.eth(),
            valid_to: model::time::now_in_epoch_seconds() + 300 + i as u32,
            kind: OrderKind::Sell,
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader1.signer,
        );

        let uid = services.create_order(&order).await.unwrap();
        trader1_uids.push(uid);
    }

    // Create 1 order for trader2 (for owner filtering testing)
    let order2 = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 3u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader2.signer,
    );

    let trader2_uid = services.create_order(&order2).await.unwrap();

    // Wait for all orders to be settled
    onchain.mint_block().await;
    let settlement_finished = || async {
        let order1 = services.get_order(&trader1_uids[0]).await.unwrap();
        let order2 = services.get_order(&trader2_uid).await.unwrap();
        !order1.metadata.executed_buy_amount.is_zero()
            && !order2.metadata.executed_buy_amount.is_zero()
    };
    wait_for_condition(TIMEOUT, settlement_finished)
        .await
        .unwrap();

    // Test 1: Get trades by order UID
    let trades_by_uid = services
        .get_trades_v2(Some(&trader1_uids[0]), None, 0, 10)
        .await
        .unwrap();

    assert_eq!(trades_by_uid.len(), 1, "Should have exactly 1 trade");
    assert_eq!(
        trades_by_uid[0].order_uid, trader1_uids[0],
        "Trade should match order UID"
    );

    // Test 2: Get trades by owner
    let trader1_trades = services
        .get_trades_v2(None, Some(&trader1.address()), 0, 100)
        .await
        .unwrap();

    assert_eq!(
        trader1_trades.len(),
        3,
        "Trader1 should have exactly 3 trades"
    );

    let trader2_trades = services
        .get_trades_v2(None, Some(&trader2.address()), 0, 10)
        .await
        .unwrap();

    assert_eq!(
        trader2_trades.len(),
        1,
        "Trader2 should have exactly 1 trade"
    );
    assert_eq!(
        trader2_trades[0].order_uid, trader2_uid,
        "Trade should match trader2's order"
    );

    // Test 3: Pagination
    // Test with limit
    let limited_trades = services
        .get_trades_v2(None, Some(&trader1.address()), 0, 2)
        .await
        .unwrap();

    assert_eq!(
        limited_trades.len(),
        2,
        "Should return exactly 2 trades with limit=2"
    );

    // Test with offset
    let offset_trades = services
        .get_trades_v2(None, Some(&trader1.address()), 1, 100)
        .await
        .unwrap();

    assert_eq!(
        offset_trades.len(),
        2,
        "Offset should skip first trade, leaving 2"
    );
}
