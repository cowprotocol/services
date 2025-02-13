use {
    e2e::{
        setup::{
            run_test,
            to_wei,
            wait_for_condition,
            Db,
            ExtraServiceArgs,
            MintableToken,
            OnchainComponents,
            Services,
            TestAccount,
            TIMEOUT,
        },
        tx,
    },
    ethrpc::Web3,
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    sqlx::Row,
    web3::{
        signing::SecretKeyRef,
        types::{H160, U256},
    },
};

#[tokio::test]
#[ignore]
async fn local_node_non_settling_solver() {
    run_test(non_settling_solver).await;
}

#[tokio::test]
#[ignore]
async fn local_node_low_settling_solver() {
    run_test(low_settling_solver).await;
}

#[tokio::test]
#[ignore]
async fn local_node_not_allowed_solver() {
    run_test(not_allowed_solver).await;
}

async fn non_settling_solver(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver, solver_b] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(1000)).await;

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
        token_a.approve(onchain.contracts().allowance, to_wei(1000))
    );

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    for _ in 0..4 {
        execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
            .await
            .unwrap();
    }

    let pool = services.db();
    let settled_auction_ids = fetch_last_settled_auction_ids(pool).await;
    assert_eq!(settled_auction_ids.len(), 4);
    // Build 5 blocks to make sure the submission deadline is passed, which is 5 by
    // default.
    for _ in 0..5 {
        onchain.mint_block().await;
    }

    // Simulate failed settlements by replacing the solver for the last 3
    // settlements.
    let last_auctions = settled_auction_ids
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    replace_solver_for_auction_ids(pool, &last_auctions, &solver_b.address()).await;
    // The competition still passes since the stats are updated only after a new
    // solution from anyone is received and stored.
    assert!(
        execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
            .await
            .is_ok()
    );
    // Now, the stat is updated, and the solver is banned.
    assert!(
        execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
            .await
            .is_err()
    );
}

async fn low_settling_solver(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver, solver_b] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(1000)).await;

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
        token_a.approve(onchain.contracts().allowance, to_wei(1000))
    );

    let services = Services::new(&onchain).await;
    let args = ExtraServiceArgs {
        // The solver is banned if the failure settlement rate is above 55%.
        autopilot: vec!["--solver-max-settlement-failure-rate=0.55".to_string()],
        ..Default::default()
    };
    services.start_protocol_with_args(args, solver).await;

    for _ in 0..5 {
        execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
            .await
            .unwrap();
    }

    let pool = services.db();
    let settled_auction_ids = fetch_last_settled_auction_ids(pool).await;
    assert_eq!(settled_auction_ids.len(), 5);
    // Build 5 blocks to make sure the submission deadline is passed, which is 5 by
    // default.
    for _ in 0..5 {
        onchain.mint_block().await;
    }

    // Simulate low settling rate by replacing the solver for the 60% of the
    // settlements.
    let random_auctions = settled_auction_ids
        .iter()
        .enumerate()
        .filter_map(|(i, id)| (i % 2 == 0).then_some(*id))
        .collect::<Vec<_>>();
    tracing::info!("newlog random_auctions={:?}", random_auctions);
    replace_solver_for_auction_ids(pool, &random_auctions, &solver_b.address()).await;
    // The competition still passes since the stats are updated only after a new
    // solution from anyone is received and stored.
    assert!(
        execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
            .await
            .is_ok()
    );
    // Now, the stat is updated, and the solver is banned.
    assert!(
        execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
            .await
            .is_err()
    );
}

async fn not_allowed_solver(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(1000)).await;

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
        token_a.approve(onchain.contracts().allowance, to_wei(1000))
    );

    let solver_address = solver.address();
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
        .await
        .unwrap();

    // Ban the solver
    onchain
        .contracts()
        .gp_authenticator
        .methods()
        .remove_solver(solver_address)
        .send()
        .await
        .unwrap();

    assert!(execute_order(&onchain, &trader_a, &token_a, &token_b, &services).await.is_err());
}

async fn replace_solver_for_auction_ids(pool: &Db, auction_ids: &[i64], solver: &H160) {
    for auction_id in auction_ids {
        sqlx::query("UPDATE settlements SET solver = $1 WHERE auction_id = $2")
            .bind(solver.0)
            .bind(auction_id)
            .execute(pool)
            .await
            .unwrap();
    }
}

async fn fetch_last_settled_auction_ids(pool: &Db) -> Vec<i64> {
    sqlx::query("SELECT auction_id FROM settlements ORDER BY auction_id DESC")
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|row| {
            let auction_id: Option<i64> = row.try_get(0).unwrap();
            auction_id
        })
        .collect()
}

async fn execute_order(
    onchain: &OnchainComponents,
    trader_a: &TestAccount,
    token_a: &MintableToken,
    token_b: &MintableToken,
    services: &Services<'_>,
) -> anyhow::Result<()> {
    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
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
    let balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);
    let auction_ids_before = fetch_last_settled_auction_ids(services.db()).await.len();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_b.balance_of(trader_a.address()).call().await.unwrap();
        let balance_changes = balance_after.checked_sub(balance_before).unwrap() >= to_wei(5);
        let auction_ids_after =
            fetch_last_settled_auction_ids(services.db()).await.len() > auction_ids_before;
        balance_changes && auction_ids_after
    })
    .await
}
