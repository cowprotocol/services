use {
    crate::setup::*,
    ethcontract::prelude::U256,
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
async fn local_node_partially_fillable() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    token_a.mint(trader_a.address(), to_wei(101)).await;
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
        token_a.approve(onchain.contracts().allowance, to_wei(101))
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
        .with_buy_amount(to_wei(80))
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

    // The following doesn't work yet because the solver crate cannot handle
    // these orders yet.
    /*
    tracing::info!("Waiting for trade.");
    services.start_old_driver(solver.private_key(), vec![]);
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert!(balance >= order_a.data.buy_amount);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { services.get_auction().await.auction.orders.is_empty() };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();
    */
}
