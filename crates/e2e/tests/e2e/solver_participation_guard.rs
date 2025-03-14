use {
    database::byte_array::ByteArray,
    e2e::{
        setup::{
            self,
            Db,
            MintableToken,
            OnchainComponents,
            Services,
            TIMEOUT,
            TestAccount,
            colocation,
            run_test,
            to_wei,
            wait_for_condition,
        },
        tx,
    },
    ethrpc::Web3,
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    std::{sync::Arc, time::Duration},
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

    let [solver_a, solver_b] = onchain.make_solvers(to_wei(1)).await;
    let (trader_a, token_a, token_b) = setup(&mut onchain, &solver_a).await;

    let services = Services::new(&onchain).await;
    // Start the upstream driver
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver_a.clone(),
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
            colocation::start_baseline_solver(
                "test_solver_2".into(),
                solver_b.clone(),
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    // The proxy drivers that forward requests to an upstream driver with some
    // additional logic.
    let proxy_driver_a = Arc::new(setup::driver::Proxy::default());
    let proxy_driver_b = Arc::new(setup::driver::Proxy::default());
    proxy_driver_b.set_upstream_base_url("http://localhost:11088/test_solver_2/".parse().unwrap());
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=test_solver|{}|{}|requested-timeout-on-problems,\
                     test_solver_2|{}|{}|requested-timeout-on-problems",
                    proxy_driver_a.url,
                    hex::encode(solver_a.address()),
                    proxy_driver_b.url,
                    hex::encode(solver_b.address())
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                // The solver gets banned for 2 settlements.
                "--solver-ban-settlements-count=2".to_string(),
                // The solver gets banned after 2 consecutive failures.
                "--non-settling-last-auctions-participation-count=2".to_string(),
                // Disable the low-settling strategy to test another non-settling properly.
                "--low-settling-solvers-blacklisting-enabled=false".to_string(),
            ],
        )
        .await;

    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Disable settling for the first solver.
    proxy_driver_a.error_on_settle_when(|_| true);
    // Exclude the second solver from the competition, so only the first solver wins
    // competitions and fails to settle them.
    proxy_driver_b.error_on_solve_when(|_| true);

    let balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    place_order(&onchain, &trader_a, &token_a, &token_b, &services).await;

    // Wait until at least 2 settle attempts are made.
    {
        let proxy_driver_a = proxy_driver_a.clone();
        let proxy_driver_b = proxy_driver_b.clone();
        wait_for_condition(TIMEOUT, || async {
            onchain.mint_block().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            // The first solver rejects all the settle calls and the second driver doesn't
            // participate in competitions.
            proxy_driver_a.get_settle_counter() >= 2 && proxy_driver_b.get_settle_counter() == 0
        })
        .await
        .unwrap();
    }

    // Make sure, no auction is settled yet.
    assert!(
        fetch_latest_settlement_solver_addresses(services.db())
            .await
            .is_empty()
    );

    // Enable the second solver to participate in the next 2 competitions.
    proxy_driver_b.error_on_solve_when(|_| false);

    // Now we wait until the second solver wins the next competition.
    // During this time, the first solver should be banned.
    let settlements_before = fetch_latest_settlement_solver_addresses(services.db())
        .await
        .len();
    wait_for_settlement(
        &onchain,
        &trader_a,
        &token_b,
        &solver_b.address(),
        &services,
        balance_before,
        settlements_before,
    )
    .await
    .unwrap();

    // The first solver should be banned already, so we can enable the settling for
    // it, which won't take any effect until unbanned.
    proxy_driver_a.error_on_settle_when(|_| false);

    // The second solver is expected to settle one more auction, so the first solver
    // is unbanned.
    execute_order(
        &onchain,
        &trader_a,
        &token_a,
        &token_b,
        &solver_b.address(),
        &services,
    )
    .await
    .unwrap();

    // After 2 settlements, the first solver should be unbanned.
    // Disable the second one from participating in the competition.
    proxy_driver_b.error_on_solve_when(|_| true);

    // The first solver settles.
    execute_order(
        &onchain,
        &trader_a,
        &token_a,
        &token_b,
        &solver_a.address(),
        &services,
    )
    .await
    .unwrap();
}

async fn low_settling_solver(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver_a, solver_b] = onchain.make_solvers(to_wei(1)).await;
    let (trader_a, token_a, token_b) = setup(&mut onchain, &solver_a).await;

    let services = Services::new(&onchain).await;
    // Start the upstream driver
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver_a.clone(),
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
            colocation::start_baseline_solver(
                "test_solver_2".into(),
                solver_b.clone(),
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    // The proxy drivers that forward requests to an upstream driver with some
    // additional logic.
    let proxy_driver_a = Arc::new(setup::driver::Proxy::default());
    let proxy_driver_b = Arc::new(setup::driver::Proxy::default());
    proxy_driver_b.set_upstream_base_url("http://localhost:11088/test_solver_2/".parse().unwrap());
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=test_solver|{}|{}|requested-timeout-on-problems,\
                     test_solver_2|{}|{}|requested-timeout-on-problems",
                    proxy_driver_a.url,
                    hex::encode(solver_a.address()),
                    proxy_driver_b.url,
                    hex::encode(solver_b.address())
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                // The solver gets banned for 2 settlements.
                "--solver-ban-settlements-count=2".to_string(),
                // Disable the non-settling strategy to test another one properly.
                "--non-settling-solvers-blacklisting-enabled=false".to_string(),
                // The solver is banned if the failure settlement rate is above 70%.
                "--solver-max-settlement-failure-rate=0.7".to_string(),
            ],
        )
        .await;

    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // That way the solver is able to settle at least 2 non-consecutive auctions.
    proxy_driver_a.error_on_settle_when(|counter| counter % 2 != 0 || counter > 5);
    // Exclude the second solver from the competition, so only the first solver wins
    // competitions and settles them.
    proxy_driver_b.error_on_solve_when(|_| true);

    for _ in 0..2 {
        execute_order(
            &onchain,
            &trader_a,
            &token_a,
            &token_b,
            &solver_a.address(),
            &services,
        )
        .await
        .unwrap();
    }

    // Since the first solver settled 2 times, to reach the 70% failure rate, we
    // need to wait for at least 7 attempts in total((1 - 2/7) > 70%).
    let balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let settlements_before = fetch_latest_settlement_solver_addresses(services.db())
        .await
        .len();
    place_order(&onchain, &trader_a, &token_a, &token_b, &services).await;
    {
        let proxy_driver_a = proxy_driver_a.clone();
        wait_for_condition(TIMEOUT, || async {
            onchain.mint_block().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            proxy_driver_a.get_settle_counter() >= 7
        })
        .await
        .unwrap();
    }

    // The first solver is banned now.
    // Enable the second solver to participate in the next 2 competitions.
    let settlements_before = {
        let current_settlements_count = fetch_latest_settlement_solver_addresses(services.db())
            .await
            .len();
        assert_eq!(current_settlements_count, settlements_before);
        current_settlements_count
    };
    // The first solver should be banned at this point. The settle can be enabled,
    // which won't take any effect until unbanned.
    proxy_driver_a.error_on_settle_when(|_| false);
    // Include the second solver in the competition to proceed with other
    // settlements.
    proxy_driver_b.error_on_solve_when(|_| false);

    // Now we wait until the second solver wins the next competition.
    wait_for_settlement(
        &onchain,
        &trader_a,
        &token_b,
        &solver_b.address(),
        &services,
        balance_before,
        settlements_before,
    )
    .await
    .unwrap();

    // The second solver is expected to settle one more auction to make the first
    // solver unbanned.
    execute_order(
        &onchain,
        &trader_a,
        &token_a,
        &token_b,
        &solver_b.address(),
        &services,
    )
    .await
    .unwrap();

    // After 2 settlements, the first solver should be unbanned.
    // Disable the second one from participating in the competition.
    proxy_driver_b.error_on_solve_when(|_| true);

    // The first solver settles.
    execute_order(
        &onchain,
        &trader_a,
        &token_a,
        &token_b,
        &solver_a.address(),
        &services,
    )
    .await
    .unwrap();
}

async fn not_allowed_solver(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let (trader_a, token_a, token_b) = setup(&mut onchain, &solver).await;

    let solver_address = solver.address();
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    execute_order(
        &onchain,
        &trader_a,
        &token_a,
        &token_b,
        &solver.address(),
        &services,
    )
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

    assert!(
        execute_order(
            &onchain,
            &trader_a,
            &token_a,
            &token_b,
            &solver.address(),
            &services
        )
        .await
        .is_err()
    );

    // Unban the solver
    onchain
        .contracts()
        .gp_authenticator
        .methods()
        .add_solver(solver_address)
        .send()
        .await
        .unwrap();

    execute_order(
        &onchain,
        &trader_a,
        &token_a,
        &token_b,
        &solver.address(),
        &services,
    )
    .await
    .unwrap();
}

async fn setup(
    onchain: &mut OnchainComponents,
    solver: &TestAccount,
) -> (TestAccount, MintableToken, MintableToken) {
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

    (trader_a, token_a, token_b)
}

async fn fetch_latest_settlement_solver_addresses(pool: &Db) -> Vec<H160> {
    let solvers: Vec<ByteArray<20>> =
        sqlx::query_as("SELECT solver FROM settlements ORDER BY auction_id DESC")
            .fetch_all(pool)
            .await
            .unwrap();

    solvers.into_iter().map(|solver| H160(solver.0)).collect()
}

async fn execute_order(
    onchain: &OnchainComponents,
    trader_a: &TestAccount,
    token_a: &MintableToken,
    token_b: &MintableToken,
    expected_solver: &H160,
    services: &Services<'_>,
) -> anyhow::Result<()> {
    let balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let settlements_before = fetch_latest_settlement_solver_addresses(services.db())
        .await
        .len();

    place_order(onchain, trader_a, token_a, token_b, services).await;

    wait_for_settlement(
        onchain,
        trader_a,
        token_b,
        expected_solver,
        services,
        balance_before,
        settlements_before,
    )
    .await
}

async fn wait_for_settlement(
    onchain: &OnchainComponents,
    trader_a: &TestAccount,
    token_b: &MintableToken,
    expected_solver: &H160,
    services: &Services<'_>,
    balance_before: U256,
    settlements_before: usize,
) -> anyhow::Result<()> {
    tracing::info!("Waiting for a settlement");
    wait_for_condition(TIMEOUT.mul_f32(1.5), || async {
        onchain.mint_block().await;
        let balance_after = token_b.balance_of(trader_a.address()).call().await.unwrap();
        let balance_changes = balance_after.checked_sub(balance_before).unwrap() >= to_wei(5);
        let new_settlements = fetch_latest_settlement_solver_addresses(services.db()).await;
        let settled_solver = new_settlements
            .first()
            .is_some_and(|solver| solver == expected_solver);
        balance_changes && settled_solver && new_settlements.len() > settlements_before
    })
    .await
}

async fn place_order(
    onchain: &OnchainComponents,
    trader_a: &TestAccount,
    token_a: &MintableToken,
    token_b: &MintableToken,
    services: &Services<'_>,
) {
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
    let order_id = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);
}
