use {
    crate::setup::*,
    ethcontract::{prelude::U256, BlockId, H160},
    model::{
        order::{OrderBuilder, OrderClass, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_partially_fillable_balance() {
    run_test(test).await;
}

/// Sets up a big partially fillable trade. Waits until 2 partial fills
/// happened and then asserts that the solver competition entries for these 2 tx
/// only contain their respectively filled amounts and fees.
async fn test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(10_000), to_wei(10_000))
        .await;

    token_a.mint(trader_a.address(), to_wei(50)).await;
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;

    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(500))
    );

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![]);
    services
        .start_api(vec![
            "--allow-placing-partially-fillable-limit-orders=true".to_string()
        ])
        .await;

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(50))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .with_partially_fillable(true)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
        )
        .build()
        .into_order_creation();
    services.create_order(&order_a).await.unwrap();

    tracing::info!("Waiting for order to show up in auction.");
    let has_order = || async { services.get_auction().await.auction.orders.len() == 1 };
    wait_for_condition(TIMEOUT, has_order).await.unwrap();

    let auction = services.get_auction().await.auction;
    let order = auction.orders.into_iter().next().unwrap();
    assert!(order.data.partially_fillable);
    assert!(matches!(order.metadata.class, OrderClass::Limit(_)));
    assert_eq!(order.metadata.full_fee_amount, 0.into());
    assert_eq!(order.metadata.solver_fee, 0.into());
    assert_eq!(auction.rewards.get(&order.metadata.uid), None);

    tracing::info!("Waiting for trade.");
    services.start_old_driver(solver.private_key(), vec!["--solvers=Baseline".to_owned()]);
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let get_block = || async {
        web3.eth()
            .block(BlockId::Number(ethcontract::BlockNumber::Latest))
            .await
            .unwrap()
            .unwrap()
    };
    let settlement_tx_1 = get_block().await.transactions.pop().unwrap();

    // Expecting a partial fill because order sells 100 but user only has 50.
    let sell_balance = token_a
        .balance_of(trader_a.address())
        .call()
        .await
        .unwrap()
        .to_f64_lossy();
    // Depending on how the solver works might not have sold all balance.
    assert!((0e18..=1e18).contains(&sell_balance));
    let buy_balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
    // We don't know exact buy balance because of the fee.
    assert!((45e18..=55e18).contains(&buy_balance.to_f64_lossy()));

    // Trader somehow gets another 25 `token_a` which allows for another partial
    // fill.
    token_a.mint(trader_a.address(), to_wei(25)).await;
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != buy_balance };
    let start = std::time::Instant::now();
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
    tracing::error!(elapsed =? start.elapsed(), "second trade done");
    let settlement_tx_2 = get_block().await.transactions.pop().unwrap();

    tracing::info!("mining blocks to get past the reorg safety threshold for indexing events");
    for _ in 0..100 {
        token_a.mint(H160::from_low_u64_be(42), 1.into()).await;
    }

    let competitions_indexed = || async {
        services
            .get_solver_competition(settlement_tx_2)
            .await
            .is_ok()
            && services
                .get_solver_competition(settlement_tx_1)
                .await
                .is_ok()
    };
    tracing::info!("waiting for solver competitions to get indexed");
    wait_for_condition(TIMEOUT, competitions_indexed)
        .await
        .unwrap();

    let competition_1 = services
        .get_solver_competition(settlement_tx_1)
        .await
        .unwrap();
    assert_eq!(competition_1.transaction_hash, Some(settlement_tx_1));
    assert_eq!(
        competition_1.common.solutions[0].objective.fees,
        113195499999999.95
    );
    assert_eq!(
        competition_1.common.solutions[0].orders[0].executed_amount,
        U256::from_f64_lossy(50e18)
    );

    let competition_2 = services
        .get_solver_competition(settlement_tx_2)
        .await
        .unwrap();
    assert_eq!(competition_2.transaction_hash, Some(settlement_tx_2));
    assert_eq!(
        competition_2.common.solutions[0].objective.fees,
        41597749999999.66
    );
    assert_eq!(
        competition_2.common.solutions[0].orders[0].executed_amount,
        U256::from_f64_lossy(25e18)
    );
}
