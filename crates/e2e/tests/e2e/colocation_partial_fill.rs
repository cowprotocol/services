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
async fn local_node_test() {
    run_test(test).await;
}

async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    // Use a shallow pool to make partial fills easier to setup.
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(10), to_wei(10))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(4))
    );
    tx_value!(
        trader.account(),
        to_wei(4),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let solver_endpoint = colocation::start_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver(onchain.contracts(), &solver_endpoint, &solver);

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![
        "--enable-colocation=true".to_string(),
        "--drivers=http://localhost:11088/test_solver".to_string(),
    ]);
    services
        .start_api(vec![
            "--allow-placing-partially-fillable-limit-orders=true".to_string()
        ])
        .await;

    tracing::info!("Placing order");
    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: to_wei(4),
        buy_token: token.address(),
        buy_amount: to_wei(3),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        partially_fillable: true,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for trade.");
    let trade_happened =
        || async { token.balance_of(trader.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // We expect the partially fillable order to only fill half-way.
    let sell_balance = onchain
        .contracts()
        .weth
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    assert!(
        // Sell balance is strictly less than 2.0 because of the fee.
        (1_999_999_000_000_000_000_u128..2_000_000_000_000_000_000_u128)
            .contains(&sell_balance.as_u128())
    );
    let buy_balance = token.balance_of(trader.address()).call().await.unwrap();
    assert!(
        (1_650_000_000_000_000_000_u128..1_670_000_000_000_000_000_u128)
            .contains(&buy_balance.as_u128())
    );

    // TODO: test that we have other important per-auction data that should have
    // made its way into the DB.
}
