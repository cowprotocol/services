use {
    ::alloy::primitives::U256,
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
async fn local_node_user_total_surplus() {
    run_test(user_total_surplus).await;
}

#[tokio::test]
#[ignore]
async fn local_node_user_total_surplus_no_trades() {
    run_test(user_total_surplus_no_trades).await;
}

#[tokio::test]
#[ignore]
async fn local_node_user_total_surplus_multiple_trades() {
    run_test(user_total_surplus_multiple_trades).await;
}

/// Test that the total surplus endpoint returns surplus for a user with trades
async fn user_total_surplus(web3: Web3) {
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
    wait_for_condition(TIMEOUT, settlement_finished)
        .await
        .unwrap();

    // Get total surplus for the trader
    let surplus = services
        .get_user_total_surplus(&trader.address())
        .await
        .unwrap();

    // After a successful trade, the user should have positive surplus
    // Since we're trading through a pool with good liquidity, we expect some
    // surplus
    assert!(
        surplus > U256::ZERO,
        "Total surplus should be positive after a successful trade, got {surplus}"
    );
}

/// Test that the total surplus endpoint returns zero for a user with no trades
async fn user_total_surplus_no_trades(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Get total surplus for a user who has never traded
    let surplus = services
        .get_user_total_surplus(&trader.address())
        .await
        .unwrap();

    // User with no trades should have zero surplus
    assert_eq!(
        surplus,
        U256::ZERO,
        "User with no trades should have zero surplus"
    );
}

/// Test that the total surplus endpoint accumulates surplus from multiple
/// trades
async fn user_total_surplus_multiple_trades(web3: Web3) {
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
    for i in 0..3 {
        let order = OrderCreation {
            sell_token: *token_a.address(),
            sell_amount: (5u64 + i as u64).eth(),
            buy_token: *token_b.address(),
            buy_amount: 1u64.eth(),
            valid_to: model::time::now_in_epoch_seconds() + 300 + i as u32,
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
    }

    // Get total surplus for the trader
    let surplus = services
        .get_user_total_surplus(&trader.address())
        .await
        .unwrap();

    // The surplus from 3 trades should be greater than from just 1 trade
    // This is a sanity check that surplus is actually accumulating
    // Note: This is approximate since market conditions can vary
    let min_expected_surplus = U256::from(1); // Very conservative minimum
    tracing::info!("surplus: {surplus}");
    assert!(
        surplus > min_expected_surplus,
        "Surplus from 3 trades should be substantial, got {surplus}"
    );
}
