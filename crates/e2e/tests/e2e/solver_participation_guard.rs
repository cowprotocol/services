use {
    alloy::primitives::{Address, U256},
    e2e::setup::{
        Db,
        ExtraServiceArgs,
        MintableToken,
        OnchainComponents,
        Services,
        TIMEOUT,
        TestAccount,
        run_test,
        wait_for_condition,
    },
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    sqlx::Row,
    std::time::Instant,
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

    let [solver, solver_b] = onchain.make_solvers(1u64.eth()).await;
    let (trader_a, token_a, token_b) = setup(&mut onchain, &solver).await;

    let services = Services::new(&onchain).await;
    let args = ExtraServiceArgs {
        autopilot: vec![
            "--non-settling-solvers-blacklisting-enabled=true".to_string(),
            "--low-settling-solvers-blacklisting-enabled=true".to_string(),
            // The solver gets banned for 40s.
            "--solver-blacklist-cache-ttl=40s".to_string(),
        ],
        ..Default::default()
    };
    services.start_protocol_with_args(args, solver).await;

    // Amount of order should be more or equal the non-settling threshold, which is
    // 3.
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
    let now = Instant::now();
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

    // 40 seconds is the cache TTL, and 5 seconds is added to compensate any
    // possible delays.
    let sleep_timeout_secs = 40 - now.elapsed().as_secs() + 5;
    println!(
        "Sleeping for {sleep_timeout_secs} seconds to reset the solver participation guard cache"
    );
    tokio::time::sleep(tokio::time::Duration::from_secs(sleep_timeout_secs)).await;
    // The cache is reset, and the solver is allowed to participate again.
    execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
        .await
        .unwrap();
}

async fn low_settling_solver(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver, solver_b] = onchain.make_solvers(1u64.eth()).await;
    let (trader_a, token_a, token_b) = setup(&mut onchain, &solver).await;

    let services = Services::new(&onchain).await;
    let args = ExtraServiceArgs {
        autopilot: vec![
            "--non-settling-solvers-blacklisting-enabled=true".to_string(),
            "--low-settling-solvers-blacklisting-enabled=true".to_string(),
            // The solver gets banned for 40s.
            "--solver-blacklist-cache-ttl=40s".to_string(),
            // The solver is banned if the failure settlement rate is above 55%.
            "--solver-max-settlement-failure-rate=0.55".to_string(),
        ],
        ..Default::default()
    };
    services.start_protocol_with_args(args, solver).await;

    // Create 5 orders, to easily test 60% of them failing, which is 3/5.
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
    replace_solver_for_auction_ids(pool, &random_auctions, &solver_b.address()).await;
    // The competition still passes since the stats are updated only after a new
    // solution from anyone is received and stored.
    let now = Instant::now();
    execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
        .await
        .unwrap();
    // Now, the stat is updated, and the solver is banned.
    execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
        .await
        .unwrap_err();

    // 40 seconds is the cache TTL, and 5 seconds is added to compensate any
    // possible delays.
    let sleep_timeout_secs = 40 - now.elapsed().as_secs() + 5;
    println!(
        "Sleeping for {sleep_timeout_secs} seconds to reset the solver participation guard cache"
    );
    tokio::time::sleep(tokio::time::Duration::from_secs(sleep_timeout_secs)).await;
    // The cache is reset, and the solver is allowed to participate again.
    execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
        .await
        .unwrap();
}

async fn not_allowed_solver(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let (trader_a, token_a, token_b) = setup(&mut onchain, &solver).await;

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
        .removeSolver(solver_address)
        .send_and_watch()
        .await
        .unwrap();

    assert!(
        execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
            .await
            .is_err()
    );

    // Unban the solver
    onchain
        .contracts()
        .gp_authenticator
        .addSolver(solver_address)
        .send_and_watch()
        .await
        .unwrap();

    execute_order(&onchain, &trader_a, &token_a, &token_b, &services)
        .await
        .unwrap();
}

async fn setup(
    onchain: &mut OnchainComponents,
    solver: &TestAccount,
) -> (TestAccount, MintableToken, MintableToken) {
    let [trader_a] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), 1000u64.eth()).await;

    // Create and fund Uniswap pool
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

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 1000u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    (trader_a, token_a, token_b)
}

async fn replace_solver_for_auction_ids(pool: &Db, auction_ids: &[i64], solver: &Address) {
    for auction_id in auction_ids {
        sqlx::query("UPDATE settlements SET solver = $1 WHERE auction_id = $2")
            .bind(solver.as_slice())
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
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    let balance_before = token_b.balanceOf(trader_a.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);
    let auction_ids_before = fetch_last_settled_auction_ids(services.db()).await.len();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_b.balanceOf(trader_a.address()).call().await.unwrap();
        let balance_changes = balance_after.checked_sub(balance_before).unwrap() >= 5u64.eth();
        let auction_ids_after =
            fetch_last_settled_auction_ids(services.db()).await.len() > auction_ids_before;
        balance_changes && auction_ids_after
    })
    .await
}
