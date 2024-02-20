use {
    e2e::{
        setup::{colocation::SolverEngine, *},
        tx,
    },
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
async fn local_node_onchain_settlement() {
    run_test(onchain_settlement).await;
}

async fn onchain_settlement(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(101)).await;
    token_b.mint(trader_b.address(), to_wei(51)).await;

    // Create and fund Uniswap pool
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

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(101))
    );
    tx!(
        trader_b.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(51))
    );

    let services = Services::new(onchain.contracts()).await;
    let solver_endpoint = colocation::start_naive_solver().await;
    colocation::start_driver(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver,
            endpoint: solver_endpoint,
        }],
    );
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let order_a = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(100).into(),
        fee_amount: to_wei(1).into(),
        buy_token: token_b.address(),
        buy_amount: to_wei(80).into(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    services.create_order(&order_a).await.unwrap();

    let order_b = OrderCreation {
        sell_token: token_b.address(),
        sell_amount: to_wei(50).into(),
        fee_amount: to_wei(1).into(),
        buy_token: token_a.address(),
        buy_amount: to_wei(40).into(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::EthSign,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    services.create_order(&order_b).await.unwrap();

    // Only start the autopilot now to ensure that these orders are settled in a
    // batch which seems to be expected in this test.
    services.start_autopilot(
        None,
        vec![
            "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ],
    );

    let balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let balance = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());

    tracing::info!("Waiting for trade.");
    let trade_happened =
        || async { token_b.balance_of(trader_a.address()).call().await.unwrap() != 0.into() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check matching
    let balance = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert!(balance >= *order_a.buy_amount);
    let balance = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert!(balance >= *order_b.buy_amount);

    tracing::info!("Waiting for auction to be cleared.");
    let auction_is_empty = || async { services.get_auction().await.auction.orders.is_empty() };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();
}
