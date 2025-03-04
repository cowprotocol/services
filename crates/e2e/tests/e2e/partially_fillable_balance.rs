use {
    e2e::{setup::*, tx},
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderKind},
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

async fn test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

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

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

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
    let order_uid = services.create_order(&order_a).await.unwrap();
    onchain.mint_block().await;
    let order = services.get_order(&order_uid).await.unwrap();
    assert!(order.is_limit_order());
    assert!(order.data.partially_fillable);

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
        !balance.is_zero()
    })
    .await
    .unwrap();

    // Expecting a partial fill because order sells 100 but user only has balance of
    // 50.
    let sell_balance = token_a
        .balance_of(trader_a.address())
        .call()
        .await
        .unwrap()
        .to_f64_lossy();
    // Depending on how the solver works might not have sold all balance.
    assert!((0e18..=1e18).contains(&sell_balance));
    let buy_balance = token_b
        .balance_of(trader_a.address())
        .call()
        .await
        .unwrap()
        .to_f64_lossy();
    // We don't know exact buy balance because of the fee.
    assert!((45e18..=55e18).contains(&buy_balance));
}
