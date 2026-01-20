use {
    bigdecimal::Zero,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::ethrpc::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_trades_v2_pagination() {
    run_test(trades_v2_pagination).await;
}

#[tokio::test]
#[ignore]
async fn local_node_trades_v2_by_order() {
    run_test(trades_v2_by_order).await;
}

#[tokio::test]
#[ignore]
async fn local_node_trades_v2_by_owner() {
    run_test(trades_v2_by_owner).await;
}

/// Test that the v2 trades endpoint returns trades with proper pagination
async fn trades_v2_pagination(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 100u64.eth()).await;

    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Create and execute multiple orders
    let mut order_uids = Vec::new();
    for i in 0..3 {
        let order = OrderCreation {
            sell_token: *token_a.address(),
            sell_amount: (2u64 + i as u64).eth(),
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
        order_uids.push(uid);
    }

    // Wait for orders to be settled
    onchain.mint_block().await;
    let settlement_finished = || async {
        let order = services.get_order(&order_uids[0]).await.unwrap();
        !order.metadata.executed_buy_amount.is_zero()
    };
    wait_for_condition(TIMEOUT, settlement_finished).await.unwrap();

    // Test pagination with offset and limit
    let all_trades = services
        .get_trades_v2(None, Some(&trader.address()), 0, 100)
        .await
        .unwrap();

    assert!(all_trades.len() >= 3, "Should have at least 3 trades");

    // Test pagination with limit
    let limited_trades = services
        .get_trades_v2(None, Some(&trader.address()), 0, 2)
        .await
        .unwrap();

    assert_eq!(limited_trades.len(), 2, "Should return exactly 2 trades with limit=2");

    // Test pagination with offset
    let offset_trades = services
        .get_trades_v2(None, Some(&trader.address()), 1, 100)
        .await
        .unwrap();

    assert_eq!(offset_trades.len(), all_trades.len() - 1, "Offset should skip first trade");
}

/// Test that the v2 trades endpoint returns trades by order UID
async fn trades_v2_by_order(web3: Web3) {
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
    wait_for_condition(TIMEOUT, settlement_finished).await.unwrap();

    // Get trades by order UID
    let trades = services
        .get_trades_v2(Some(&uid), None, 0, 10)
        .await
        .unwrap();

    assert_eq!(trades.len(), 1, "Should have exactly 1 trade");
    assert_eq!(trades[0].order_uid, uid, "Trade should match order UID");
}

/// Test that the v2 trades endpoint returns trades by owner
async fn trades_v2_by_owner(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader1, trader2] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Mint tokens for both traders
    token_a.mint(trader1.address(), 10u64.eth()).await;
    token_a.mint(trader2.address(), 10u64.eth()).await;

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
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

    // Create order for trader1
    let order1 = OrderCreation {
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
        &trader1.signer,
    );

    let uid1 = services.create_order(&order1).await.unwrap();

    // Create order for trader2
    let order2 = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 3u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 301,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader2.signer,
    );

    let uid2 = services.create_order(&order2).await.unwrap();

    // Wait for orders to be settled
    onchain.mint_block().await;
    let settlement_finished = || async {
        let order1 = services.get_order(&uid1).await.unwrap();
        let order2 = services.get_order(&uid2).await.unwrap();
        !order1.metadata.executed_buy_amount.is_zero()
            && !order2.metadata.executed_buy_amount.is_zero()
    };
    wait_for_condition(TIMEOUT, settlement_finished).await.unwrap();

    // Get trades for trader1
    let trader1_trades = services
        .get_trades_v2(None, Some(&trader1.address()), 0, 10)
        .await
        .unwrap();

    assert_eq!(trader1_trades.len(), 1, "Trader1 should have exactly 1 trade");
    assert_eq!(
        trader1_trades[0].order_uid, uid1,
        "Trade should match trader1's order"
    );

    // Get trades for trader2
    let trader2_trades = services
        .get_trades_v2(None, Some(&trader2.address()), 0, 10)
        .await
        .unwrap();

    assert_eq!(trader2_trades.len(), 1, "Trader2 should have exactly 1 trade");
    assert_eq!(
        trader2_trades[0].order_uid, uid2,
        "Trade should match trader2's order"
    );
}
