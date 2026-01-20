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
};

#[tokio::test]
#[ignore]
async fn local_node_single_order_cancellation() {
    run_test(single_order_cancellation).await;
}

#[tokio::test]
#[ignore]
async fn local_node_solver_competition_v1_by_auction_id() {
    run_test(solver_competition_v1_by_auction_id).await;
}

#[tokio::test]
#[ignore]
async fn local_node_solver_competition_v1_by_tx_hash() {
    run_test(solver_competition_v1_by_tx_hash).await;
}

#[tokio::test]
#[ignore]
async fn local_node_solver_competition_v1_latest() {
    run_test(solver_competition_v1_latest).await;
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

/// Test the deprecated v1 solver competition endpoint by auction ID
async fn solver_competition_v1_by_auction_id(web3: Web3) {
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

    // Wait for solver competition to be indexed and get the auction ID
    for _ in 0..3 {
        onchain.mint_block().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    let indexed = || async { services.get_latest_solver_competition_v1().await.is_ok() };
    wait_for_condition(TIMEOUT, indexed).await.unwrap();

    // Get latest competition to extract auction ID
    let latest_competition = services.get_latest_solver_competition_v1().await.unwrap();
    let auction_id = latest_competition.auction_id;

    // Get solver competition using v1 endpoint by auction ID
    let competition = services
        .get_solver_competition_v1(auction_id)
        .await
        .unwrap();

    // Verify the competition data is returned and matches the auction ID
    assert_eq!(
        competition.auction_id, auction_id,
        "Auction ID should match"
    );
    assert!(
        !competition.common.solutions.is_empty(),
        "Competition should have at least one solution"
    );
}

/// Test the deprecated v1 solver competition endpoint by transaction hash
async fn solver_competition_v1_by_tx_hash(web3: Web3) {
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

    // Get trade to extract transaction hash
    let trades = services.get_trades(&uid).await.unwrap();
    assert!(!trades.is_empty(), "Should have at least one trade");

    let tx_hash = trades[0].tx_hash.expect("Trade should have tx_hash");

    // Wait for solver competition to be indexed
    // Mint a few blocks to ensure indexing completes
    for _ in 0..3 {
        onchain.mint_block().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    let indexed = || async {
        services
            .get_solver_competition_by_tx_v1(tx_hash)
            .await
            .is_ok()
    };
    wait_for_condition(TIMEOUT, indexed).await.unwrap();

    // Get solver competition using v1 endpoint
    let competition = services
        .get_solver_competition_by_tx_v1(tx_hash)
        .await
        .unwrap();

    // Verify the competition data is returned
    assert!(
        !competition.common.solutions.is_empty(),
        "Competition should have at least one solution"
    );
}

/// Test the deprecated v1 latest solver competition endpoint
async fn solver_competition_v1_latest(web3: Web3) {
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

    // Wait for solver competition to be indexed
    // Mint a few blocks to ensure indexing completes
    for _ in 0..3 {
        onchain.mint_block().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    let indexed = || async { services.get_latest_solver_competition_v1().await.is_ok() };
    wait_for_condition(TIMEOUT, indexed).await.unwrap();

    // Get latest solver competition using v1 endpoint
    let competition = services.get_latest_solver_competition_v1().await.unwrap();

    // Verify the competition data is returned
    assert!(
        !competition.common.solutions.is_empty(),
        "Latest competition should have at least one solution"
    );
}
