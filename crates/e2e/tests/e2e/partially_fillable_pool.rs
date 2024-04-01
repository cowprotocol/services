use {
    e2e::{setup::*, tx},
    ethcontract::prelude::U256,
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
async fn local_node_partially_fillable_pool() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    token_a.mint(trader_a.address(), to_wei(500)).await;
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
    services.start_protocol(solver).await;

    let order_a = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(500),
        buy_token: token_b.address(),
        buy_amount: to_wei(390),
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
    assert!(order.partially_fillable);
    assert!(matches!(order.class, OrderClass::Limit));

    tracing::info!("Waiting for trade.");
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Expecting a partial fill because the pool cannot trade the full amount.
    let sell_balance = token_a.balance_of(trader_a.address()).call().await.unwrap();
    assert!(
        // Sell balance is strictly less than 250.0 because of the fee.
        (249_999_000_000_000_000_000_u128..250_000_000_000_000_000_000_u128)
            .contains(&sell_balance.as_u128())
    );
    let buy_balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert!(
        (199_000_000_000_000_000_000_u128..201_000_000_000_000_000_000_u128)
            .contains(&buy_balance.as_u128())
    );

    let metadata_updated = || async {
        onchain.mint_block().await;
        let order = services.get_order(&uid).await.unwrap();
        !order.metadata.executed_surplus_fee.is_zero()
            && order.metadata.executed_buy_amount != Default::default()
            && order.metadata.executed_sell_amount != Default::default()
    };
    wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();
}
