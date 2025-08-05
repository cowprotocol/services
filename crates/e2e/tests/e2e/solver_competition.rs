use {
    e2e::{
        setup::{colocation::SolverEngine, mock::Mock, *},
        tx,
    },
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    solvers_dto::solution::Solution,
    std::collections::HashMap,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_solver_competition() {
    run_test(solver_competition).await;
}

#[tokio::test]
#[ignore]
async fn local_node_wrong_solution_submission_address() {
    run_test(wrong_solution_submission_address).await;
}

#[tokio::test]
#[ignore]
async fn local_node_store_filtered_solutions() {
    run_test(store_filtered_solutions).await;
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
                1,
                true,
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver.clone(),
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

    let services = Services::new(&onchain).await;
    services.start_autopilot(
        None,
        vec![
            format!("--drivers=test_solver|http://localhost:11088/test_solver|{},solver2|http://localhost:11088/solver2|{}", hex::encode(solver.address()), hex::encode(solver.address())
            ),
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

    assert!(competition.solutions.len() == 2);

    // Non winning candidate
    assert!(competition.solutions[0].ranking == 2);
    assert!(!competition.solutions[0].is_winner);
    // Winning candidate
    assert!(competition.solutions[1].ranking == 1);
    assert!(competition.solutions[1].is_winner);
}

async fn wrong_solution_submission_address(web3: Web3) {
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
                1,
                true,
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver.clone(),
                onchain.contracts().weth.address(),
                vec![base_b.address()],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;
    services.start_autopilot(
        None,
        // Solver 1 has a wrong submission address, meaning that the solutions should be discarded from solver1
        vec![
            format!("--drivers=solver1|http://localhost:11088/test_solver|0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2,solver2|http://localhost:11088/solver2|{}", hex::encode(solver.address())),
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
        ],
    ).await;
    services
        .start_api(vec![
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

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

    onchain.mint_block().await;

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

    // Verify that test_solver was excluded due to wrong driver address
    let trades = services.get_trades(&uid_a).await.unwrap();
    let competition = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .unwrap();
    tracing::info!(?competition, "competition");
    assert_eq!(
        competition.solutions.last().unwrap().solver_address,
        solver.address()
    );
    assert_eq!(competition.solutions.len(), 1);
}

async fn store_filtered_solutions(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [good_solver_account, bad_solver_account] = onchain.make_solvers(to_wei(100)).await;
    let [trader] = onchain.make_accounts(to_wei(100)).await;
    let [token_a, token_b, token_c] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(300_000), to_wei(1_000))
        .await;

    // give the settlement contract a ton of the traded tokens so that the mocked
    // solver solutions can simply give money away to make the trade execute
    token_b
        .mint(onchain.contracts().gp_settlement.address(), to_wei(50))
        .await;
    token_c
        .mint(onchain.contracts().gp_settlement.address(), to_wei(50))
        .await;

    // set up trader for their order
    token_a.mint(trader.address(), to_wei(2)).await;
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(2))
    );

    let services = Services::new(&onchain).await;

    let good_solver = Mock::default();
    let bad_solver = Mock::default();

    // Start system
    let base_tokens = vec![token_a.address(), token_b.address(), token_c.address()];
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                good_solver_account.clone(),
                onchain.contracts().weth.address(),
                base_tokens.clone(),
                1,
                true,
            )
            .await,
            SolverEngine {
                name: "good_solver".into(),
                account: good_solver_account.clone(),
                endpoint: good_solver.url.clone(),
                base_tokens: base_tokens.clone(),
                merge_solutions: true,
            },
            SolverEngine {
                name: "bad_solver".into(),
                account: bad_solver_account.clone(),
                endpoint: bad_solver.url.clone(),
                base_tokens,
                merge_solutions: true,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    // We start the quoter as the baseline solver, and the mock solver as the one
    // returning the solution
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=good_solver|http://localhost:11088/good_solver|{},bad_solver|http://localhost:11088/bad_solver|{}",
                    hex::encode(good_solver_account.address()),
                    hex::encode(bad_solver_account.address()),
                ),
                "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver"
                    .to_string(),
                "--max-winners-per-auction=10".to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Place order
    let order_ab = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(1),
        buy_token: token_b.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let order_ac = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(1),
        buy_token: token_c.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let order_ab_id = services.create_order(&order_ab).await.unwrap();
    let order_ac_id = services.create_order(&order_ac).await.unwrap();
    onchain.mint_block().await;

    // good solver settles order_ab at a price 3:1
    good_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (token_a.address(), to_wei(3)),
            (token_b.address(), to_wei(1)),
        ]),
        trades: vec![solvers_dto::solution::Trade::Fulfillment(
            solvers_dto::solution::Fulfillment {
                executed_amount: order_ab.sell_amount,
                fee: Some(0.into()),
                order: solvers_dto::solution::OrderUid(order_ab_id.0),
            },
        )],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
    }));

    // bad solver settles both orders at 2:1. Because it can't beat the
    // reference solution of order_a provided by the good solver this
    // solution will get filtered during the combinatorial auction.
    bad_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (token_a.address(), to_wei(2)),
            (token_b.address(), to_wei(1)),
            (token_c.address(), to_wei(1)),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order_ab.sell_amount,
                fee: Some(0.into()),
                order: solvers_dto::solution::OrderUid(order_ab_id.0),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order_ac.sell_amount,
                fee: Some(0.into()),
                order: solvers_dto::solution::OrderUid(order_ac_id.0),
            }),
        ],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
    }));

    // Drive solution
    tracing::info!("Waiting for trade to get indexed");
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let trade = services.get_trades(&order_ab_id).await.unwrap().pop()?;
        Some(
            services
                .get_solver_competition(trade.tx_hash?)
                .await
                .is_ok(),
        )
    })
    .await
    .unwrap();

    let trade = services
        .get_trades(&order_ab_id)
        .await
        .unwrap()
        .pop()
        .unwrap();

    let competition = services
        .get_solver_competition(trade.tx_hash.unwrap())
        .await
        .unwrap();

    assert_eq!(competition.transaction_hashes.len(), 1);
    assert_eq!(competition.transaction_hashes[0], trade.tx_hash.unwrap());

    assert_eq!(competition.reference_scores.len(), 1);
    // since the only other solutions were unfair the reference score is zero
    assert_eq!(
        competition
            .reference_scores
            .get(&good_solver_account.address()),
        Some(&0.into())
    );

    assert_eq!(competition.solutions.len(), 2);

    // check that JSON endpoint contains the filtered solution
    let bad_solution = &competition.solutions[0];
    assert_eq!(bad_solution.ranking, 2);
    assert!(bad_solution.filtered_out);
    assert!(!bad_solution.is_winner);
    assert_eq!(bad_solution.solver_address, bad_solver_account.address());
    assert!(bad_solution.tx_hash.is_none());
    assert!(bad_solution.reference_score.is_none());

    let good_solution = &competition.solutions[1];
    assert_eq!(good_solution.ranking, 1);
    assert!(!good_solution.filtered_out);
    assert!(good_solution.is_winner);
    assert_eq!(good_solution.solver_address, good_solver_account.address());
    assert_eq!(good_solution.tx_hash.unwrap(), trade.tx_hash.unwrap());
    // since the only other solutions were unfair the reference score is zero
    assert_eq!(good_solution.reference_score, Some(0.into()));

    // check that new DB tables contain the filtered solution
    let mut db = services.db().acquire().await.unwrap();
    let solutions = database::solver_competition_v2::fetch(&mut db, competition.auction_id)
        .await
        .unwrap();
    assert!(
        solutions.iter().any(|s| s.filtered_out
            && !s.is_winner
            && s.solver.0 == bad_solver_account.address().0)
    );
    assert!(
        solutions.iter().any(|s| !s.filtered_out
            && s.is_winner
            && s.solver.0 == good_solver_account.address().0)
    );
}
