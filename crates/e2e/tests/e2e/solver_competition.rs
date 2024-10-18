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
async fn local_node_solver_competition() {
    run_test(solver_competition).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fairness_check() {
    run_test(fairness_check).await;
}

async fn solver_competition(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(trader.address(), to_wei(10)).await;
    token_a.mint(solver.address(), to_wei(1000)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(100))
    );

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                onchain.contracts().weth.address(),
                vec![],
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver,
                onchain.contracts().weth.address(),
                vec![],
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    let services = Services::new(&onchain).await;
    services.start_autopilot(
        None,
        vec![
            "--drivers=test_solver|http://localhost:11088/test_solver,solver2|http://localhost:11088/solver2"
                .to_string(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver,solver2|http://localhost:11088/solver2".to_string(),
        ],
    ).await;
    services.start_api(vec![
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver,solver2|http://localhost:11088/solver2".to_string(),
    ]).await;

    // Place Order
    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let uid = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    tracing::info!("waiting for trade");
    let trade_happened =
        || async { token_a.balance_of(trader.address()).call().await.unwrap() == U256::zero() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let indexed_trades = || async {
        onchain.mint_block().await;
        match services.get_trades(&uid).await.unwrap().first() {
            Some(trade) => services
                .get_solver_competition(trade.tx_hash.unwrap())
                .await
                .is_ok(),
            None => false,
        }
    };
    wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();

    let trades = services.get_trades(&uid).await.unwrap();
    let competition = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .unwrap();

    assert!(competition.common.solutions.len() == 2);

    // Non winning candidate
    assert!(competition.common.solutions[0].ranking == 2);
    // Winning candidate
    assert!(competition.common.solutions[1].ranking == 1);
}

async fn fairness_check(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund traders
    token_a.mint(trader_a.address(), to_wei(10)).await;
    token_b.mint(trader_b.address(), to_wei(10)).await;

    // Create more liquid routes between token_a (token_b) and weth via base_a
    // (base_b). base_a has more liquidity then base_b, leading to the solver that
    // knows about base_a to win
    let [base_a, base_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(10_000), to_wei(10_000))
        .await;
    onchain
        .seed_uni_v2_pool((&token_a, to_wei(100_000)), (&base_a, to_wei(100_000)))
        .await;
    onchain
        .seed_uni_v2_pool((&token_b, to_wei(10_000)), (&base_b, to_wei(10_000)))
        .await;

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(100))
    );
    tx!(
        trader_b.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(100))
    );

    // Start system, with two solvers, one that knows about base_a and one that
    // knows about base_b
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                onchain.contracts().weth.address(),
                vec![base_a.address()],
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver.clone(),
                onchain.contracts().weth.address(),
                vec![base_b.address()],
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    let services = Services::new(&onchain).await;
    services.start_autopilot(
        None,
        // Solver 1 has a fairness threshold of 0.01 ETH, which should be triggered by sub-optimally settling order_b
        vec![
            "--drivers=solver1|http://localhost:11088/test_solver|10000000000000000,solver2|http://localhost:11088/solver2"
                .to_string(),
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
        ],
    ).await;
    services
        .start_api(vec![
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Disable solving until all orders have been placed.
    onchain.allow_solving(&solver, false).await;

    // Place Orders
    let order_a = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let uid_a = services.create_order(&order_a).await.unwrap();

    let order_b = OrderCreation {
        sell_token: token_b.address(),
        sell_amount: to_wei(10),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    services.create_order(&order_b).await.unwrap();

    // Enable solving again
    onchain.allow_solving(&solver, true).await;

    // Wait for trade
    let indexed_trades = || async {
        onchain.mint_block().await;
        match services.get_trades(&uid_a).await.unwrap().first() {
            Some(trade) => services
                .get_solver_competition(trade.tx_hash.unwrap())
                .await
                .is_ok(),
            None => false,
        }
    };
    wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();

    // Verify that test_solver was excluded due to fairness check
    let trades = services.get_trades(&uid_a).await.unwrap();
    let competition = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .unwrap();
    tracing::info!(?competition, "competition");
    assert_eq!(
        competition.common.solutions.last().unwrap().solver,
        "solver2"
    );
    assert_eq!(competition.common.solutions.len(), 1);
}
