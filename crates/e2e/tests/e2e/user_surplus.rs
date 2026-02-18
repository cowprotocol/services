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
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_user_surplus_endpoint() {
    run_test(user_surplus_endpoint).await;
}

/// Test the user surplus endpoint with no trades, single trade, and multiple
/// trades
async fn user_surplus_endpoint(web3: Web3) {
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

    // Test 1: User with no trades should have zero surplus
    let surplus_before = services
        .get_user_total_surplus(&trader.address())
        .await
        .unwrap();

    assert_eq!(
        surplus_before,
        U256::ZERO,
        "User with no trades should have zero surplus"
    );

    // Create and execute first order
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
        &trader.signer,
    );

    let uid1 = services.create_order(&order1).await.unwrap();

    // Wait for first order to be settled
    onchain.mint_block().await;
    let settlement_finished = || async {
        let order = services.get_order(&uid1).await.unwrap();
        !order.metadata.executed_buy_amount.is_zero()
    };
    wait_for_condition(TIMEOUT, settlement_finished)
        .await
        .unwrap();

    // Wait for solver competition data to be indexed
    let indexed_trades = || async {
        match services.get_trades(&uid1).await.unwrap().first() {
            Some(trade) => services
                .get_solver_competition(trade.tx_hash.unwrap())
                .await
                .is_ok(),
            None => false,
        }
    };
    wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();

    // Test 2: User with one trade should have positive surplus
    let surplus_after_one = services
        .get_user_total_surplus(&trader.address())
        .await
        .unwrap();

    assert!(
        surplus_after_one > U256::ZERO,
        "Surplus should be positive after one trade, got {surplus_after_one}"
    );

    // Create and execute two more orders
    for i in 1..3 {
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

        // Wait for solver competition data to be indexed
        let indexed_trades = || async {
            match services.get_trades(&uid).await.unwrap().first() {
                Some(trade) => services
                    .get_solver_competition(trade.tx_hash.unwrap())
                    .await
                    .is_ok(),
                None => false,
            }
        };
        wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();
    }

    // Test 3: User with multiple trades should have accumulated surplus
    let surplus_after_three = services
        .get_user_total_surplus(&trader.address())
        .await
        .unwrap();

    assert!(
        surplus_after_three > surplus_after_one,
        "Surplus should accumulate across trades: {surplus_after_one} -> {surplus_after_three}"
    );
}
