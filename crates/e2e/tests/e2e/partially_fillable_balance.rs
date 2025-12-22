use {
    ::alloy::primitives::U256,
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
async fn local_node_partially_fillable_balance() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader_a] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader_a.address(), 50u64.eth()).await;
    token_a.mint(solver.address(), 1000u64.eth()).await;
    token_b.mint(solver.address(), 1000u64.eth()).await;

    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            1000u64.eth(),
            1000u64.eth(),
            U256::ZERO,
            U256::ZERO,
            solver.address(),
            U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(onchain.contracts().allowance, 500u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order_a = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 100u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 50u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: true,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    let order_uid = services.create_order(&order_a).await.unwrap();
    onchain.mint_block().await;
    let order = services.get_order(&order_uid).await.unwrap();
    assert!(order.is_limit_order());
    assert!(order.data.partially_fillable);

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance = token_b.balanceOf(trader_a.address()).call().await.unwrap();
        !balance.is_zero()
    })
    .await
    .unwrap();

    // Expecting a partial fill because order sells 100 but user only has balance of
    // 50.
    let sell_balance = token_a.balanceOf(trader_a.address()).call().await.unwrap();
    // Depending on how the solver works might not have sold all balance.
    assert!(U256::ZERO <= sell_balance && sell_balance < U256::from(10u64.pow(18)));
    let buy_balance = token_b.balanceOf(trader_a.address()).call().await.unwrap();
    // We don't know exact buy balance because of the fee.
    assert!(
        U256::from(45) * U256::from(10u64.pow(18)) <= buy_balance
            && buy_balance <= U256::from(55) * U256::from(10u64.pow(18))
    );
}
