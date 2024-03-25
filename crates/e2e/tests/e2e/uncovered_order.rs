use {
    e2e::{setup::*, tx, tx_value},
    ethcontract::U256,
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
async fn local_node_uncovered_order() {
    run_test(test).await;
}

/// Tests that a user can already create an order if they only have
/// 1 wei of the sell token and later fund their account to get the
/// order executed.
async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;
    let weth = &onchain.contracts().weth;

    tx!(
        trader.account(),
        weth.approve(onchain.contracts().allowance, to_wei(3))
    );

    tracing::info!("Starting services.");
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    tracing::info!("Placing order with 0 sell tokens");
    let order = OrderCreation {
        sell_token: weth.address(),
        sell_amount: to_wei(2),
        fee_amount: 0.into(),
        buy_token: token.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap_err();

    tracing::info!("Placing order with 1 wei of sell_tokens");
    tx_value!(trader.account(), 1.into(), weth.deposit());
    services.create_order(&order).await.unwrap();

    tracing::info!("Deposit ETH to make order executable");
    tx_value!(trader.account(), to_wei(2), weth.deposit());

    tracing::info!("Waiting for order to show up in auction");
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();
    let balance_after = weth.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(U256::one(), balance_after);
}
