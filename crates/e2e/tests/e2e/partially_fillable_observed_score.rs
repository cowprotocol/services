use {
    e2e::{setup::*, tx},
    ethcontract::prelude::U256,
    futures::StreamExt,
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_partially_fillable_observed_score() {
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

    let order_a = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(100),
        buy_token: token_b.address(),
        buy_amount: to_wei(50),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: true,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let uid = services.create_order(&order_a).await.unwrap();

    tracing::info!("Waiting for order to show up in auction.");
    let has_order = || async { services.get_auction().await.auction.orders.len() == 1 };
    wait_for_condition(TIMEOUT, has_order).await.unwrap();

    let auction = services.get_auction().await.auction;
    let order = auction.orders.into_iter().next().unwrap();
    assert!(order.data.partially_fillable);
    assert!(matches!(order.metadata.class, OrderClass::Limit(_)));
    assert_eq!(order.metadata.full_fee_amount, 0.into());
    assert_eq!(order.metadata.solver_fee, 0.into());

    services.start_old_driver(solver.private_key(), vec!["--solvers=Baseline".to_owned()]);

    tracing::info!("Waiting for trade.");
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
    let balance = token_b.balance_of(trader_a.address()).call().await.unwrap();

    // Add balance so that second trade happens.
    token_a.mint(trader_a.address(), to_wei(25)).await;
    tracing::info!("Waiting for trade.");
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != balance };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    onchain.mint_blocks_past_reorg_threshold().await;

    let indexed_trades = || async { services.get_trades(&uid).await.unwrap().len() == 2 };
    wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();
    let trades = services.get_trades(&uid).await.unwrap();

    tracing::info!("waiting for solver competitions to get indexed");
    let competitions_indexed = || {
        futures::stream::iter(&trades).all(|trade| async {
            services
                .get_solver_competition(trade.tx_hash.unwrap())
                .await
                .is_ok()
        })
    };
    wait_for_condition(TIMEOUT, competitions_indexed)
        .await
        .unwrap();

    let competition_0 = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .unwrap();
    let competition_1 = services
        .get_solver_competition(trades[1].tx_hash.unwrap())
        .await
        .unwrap();

    tracing::info!(?trades, ?competition_0, ?competition_1);

    assert_eq!(competition_0.common.solutions.len(), 1);
    assert_eq!(competition_1.common.solutions.len(), 1);
    let solution_0 = &competition_0.common.solutions[0];
    let solution_1 = &competition_1.common.solutions[0];

    assert!(solution_0.objective.fees > 0.);
    assert!(solution_1.objective.fees > 0.);
    assert_ne!(solution_0.objective.fees, solution_1.objective.fees);

    assert!(solution_0.orders[0].executed_amount > 0.into());
    assert!(solution_1.orders[0].executed_amount > 0.into());
    assert_ne!(
        solution_0.orders[0].executed_amount,
        solution_1.orders[0].executed_amount
    );
}
