use {
    ::alloy::primitives::U256,
    e2e::setup::{eth, *},
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
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

    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            eth(1000),
            eth(1000),
            U256::ZERO,
            U256::ZERO,
            solver.address().into_alloy(),
            U256::MAX,
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(onchain.contracts().allowance.into_alloy(), eth(500))
        .from(trader_a.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;
    onchain.mint_block().await;

    let order_a = OrderCreation {
        sell_token: token_a.address().into_legacy(),
        sell_amount: to_wei(500),
        buy_token: token_b.address().into_legacy(),
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
    let order = services.get_order(&uid).await.unwrap();
    assert!(order.is_limit_order());
    assert!(order.data.partially_fillable);

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance = token_b
            .balanceOf(trader_a.address().into_alloy())
            .call()
            .await
            .unwrap();
        onchain.mint_block().await;
        !balance.is_zero()
    })
    .await
    .unwrap();

    // Expecting a partial fill because the pool cannot trade the full amount.
    let sell_balance = token_a
        .balanceOf(trader_a.address().into_alloy())
        .call()
        .await
        .unwrap();
    assert!(
        // Sell balance is strictly less than 250.0 because of the fee.
        (249_999_000_000_000_000_000_u128..250_000_000_000_000_000_000_u128)
            .contains(&u128::try_from(sell_balance).unwrap())
    );
    let buy_balance = token_b
        .balanceOf(trader_a.address().into_alloy())
        .call()
        .await
        .unwrap();
    assert!(
        (199_000_000_000_000_000_000_u128..201_000_000_000_000_000_000_u128)
            .contains(&u128::try_from(buy_balance).unwrap())
    );

    let metadata_updated = || async {
        let order = services.get_order(&uid).await.unwrap();
        onchain.mint_block().await;
        !order.metadata.executed_fee.is_zero()
            && order.metadata.executed_buy_amount != Default::default()
            && order.metadata.executed_sell_amount != Default::default()
    };
    wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();
}
