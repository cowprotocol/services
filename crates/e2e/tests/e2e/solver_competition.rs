use {
    ::alloy::primitives::{U256, address},
    configs::{
        autopilot::{
            Configuration,
            run_loop::RunLoopConfig,
            solver::{Account, Solver},
        },
        order_quoting::{ExternalSolver, OrderQuoting},
        test_util::TestDefault,
    },
    e2e::setup::{colocation::SolverEngine, mock::Mock, *},
    ethrpc::alloy::{CallBuilderExt, EvmProviderExt},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    reqwest::StatusCode,
    shared::web3::Web3,
    solvers_dto::solution::Solution,
    std::{collections::HashMap, str::FromStr},
    url::Url,
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

#[tokio::test]
#[ignore]
async fn local_node_cannot_replace_order_bid_on_by_non_winning_solution() {
    run_test(cannot_replace_order_bid_on_by_non_winning_solution).await;
}

async fn solver_competition(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(trader.address(), 10u64.eth()).await;
    token_a.mint(solver.address(), 1000u64.eth()).await;

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
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

    let base_config = Configuration::test_no_drivers();
    services
        .start_autopilot(
            None,
            Configuration {
                drivers: vec![
                    Solver::test("test_solver", solver.address()),
                    Solver::test("solver2", solver.address()),
                ],
                order_quoting: OrderQuoting::test_with_drivers(vec![
                    ExternalSolver::new("test_quoter", "http://localhost:11088/test_solver"),
                    ExternalSolver::new("solver2", "http://localhost:11088/solver2"),
                ]),
                run_loop: RunLoopConfig {
                    submission_deadline: 3,
                    ..base_config.run_loop
                },
                ..base_config
            },
        )
        .await;
    services
        .start_api(configs::orderbook::Configuration {
            hide_competition_before_deadline: true,
            order_quoting: OrderQuoting::test_with_drivers(vec![
                ExternalSolver::new("test_quoter", "http://localhost:11088/test_solver"),
                ExternalSolver::new("solver2", "http://localhost:11088/solver2"),
            ]),
            ..configs::orderbook::Configuration::test_default()
        })
        .await;

    // Place Order
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;

    tracing::info!("waiting for trade");
    let trade_happened = || async {
        token_a
            .balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Competition data is saved before the settlement tx, so it is already in
    // the DB. The deadline hasn't passed yet → 404.
    assert_eq!(
        services.get_latest_solver_competition().await.unwrap_err(),
        StatusCode::NOT_FOUND,
    );

    // The internal (unfiltered) endpoint returns the data regardless.
    let auction_id: i64 = {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar("SELECT id FROM competition_auctions ORDER BY id DESC LIMIT 1")
            .fetch_one(&mut *db)
            .await
            .unwrap()
    };
    assert!(
        services
            .get_solver_competition_unfiltered(auction_id)
            .await
            .is_ok()
    );

    // The indexed_trades poll mints a block on every iteration, which will
    // advance past the 3-block deadline while also waiting for the event
    // indexer to pick up the settlement.
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

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund traders
    token_a.mint(trader_a.address(), 10u64.eth()).await;
    token_b.mint(trader_b.address(), 10u64.eth()).await;

    // Create more liquid routes between token_a (token_b) and weth via base_a
    // (base_b). base_a has more liquidity then base_b, leading to the solver that
    // knows about base_a to win
    let [base_a, base_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(10_000u64.eth(), 10_000u64.eth())
        .await;
    onchain
        .seed_uni_v2_pool((&token_a, 100_000u64.eth()), (&base_a, 100_000u64.eth()))
        .await;
    onchain
        .seed_uni_v2_pool((&token_b, 10_000u64.eth()), (&base_b, 10_000u64.eth()))
        .await;

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader_b.address())
        .send_and_watch()
        .await
        .unwrap();

    // Start system, with two solvers, one that knows about base_a and one that
    // knows about base_b
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![*base_a.address()],
                1,
                true,
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![*base_b.address()],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;

    services
        .start_autopilot(
            None,
            Configuration {
                drivers: vec![
                    // Solver 1 has a wrong submission address, meaning that the solutions should
                    // be discarded from solver1
                    Solver::new(
                        "solver1".to_string(),
                        Url::from_str("http://localhost:11088/test_solver").unwrap(),
                        Account::Address(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
                    ),
                    Solver::test("solver2", solver.address()),
                ],
                order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                    "solver1",
                    "http://localhost:11088/test_solver",
                )]),
                ..Configuration::test_no_drivers()
            },
        )
        .await;
    services
        .start_api(configs::orderbook::Configuration {
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "solver1",
                "http://localhost:11088/test_solver",
            )]),
            ..configs::orderbook::Configuration::test_default()
        })
        .await;

    // Place Orders
    let order_a = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
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
    let uid_a = services.create_order(&order_a).await.unwrap();

    onchain.mint_block().await;

    let order_b = OrderCreation {
        sell_token: *token_b.address(),
        sell_amount: 10u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_b.signer,
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

    let [good_solver_account, bad_solver_account] = onchain.make_solvers(100u64.eth()).await;
    let [trader] = onchain.make_accounts(100u64.eth()).await;
    let [token_a, token_b, token_c] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(300_000u64.eth(), 1_000u64.eth())
        .await;

    // give the settlement contract a ton of the traded tokens so that the mocked
    // solver solutions can simply give money away to make the trade execute
    token_b
        .mint(*onchain.contracts().gp_settlement.address(), 50u64.eth())
        .await;
    token_c
        .mint(*onchain.contracts().gp_settlement.address(), 50u64.eth())
        .await;

    // set up trader for their order
    token_a.mint(trader.address(), 2u64.eth()).await;

    token_a
        .approve(onchain.contracts().allowance, 2u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;

    let good_solver = Mock::new().await;
    let bad_solver = Mock::new().await;

    // Start system
    let base_tokens = vec![*token_a.address(), *token_b.address(), *token_c.address()];
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                good_solver_account.clone(),
                *onchain.contracts().weth.address(),
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
                haircut_bps: 0,
                submission_keys: vec![],
            },
            SolverEngine {
                name: "bad_solver".into(),
                account: bad_solver_account.clone(),
                endpoint: bad_solver.url.clone(),
                base_tokens,
                merge_solutions: true,
                haircut_bps: 0,
                submission_keys: vec![],
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    // We start the quoter as the baseline solver, and the mock solver as the one
    // returning the solution
    let config = Configuration::test_no_drivers();
    services
        .start_autopilot(
            None,
            Configuration {
                drivers: vec![
                    Solver::test("good_solver", good_solver_account.address()),
                    Solver::test("bad_solver", bad_solver_account.address()),
                ],
                order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                    "test_solver",
                    "http://localhost:11088/test_solver",
                )]),
                run_loop: RunLoopConfig {
                    max_winners_per_auction: std::num::NonZeroUsize::new(10).unwrap(),
                    ..config.run_loop
                },
                ..config
            },
        )
        .await;
    services
        .start_api(configs::orderbook::Configuration {
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "test_solver",
                "http://localhost:11088/test_solver",
            )]),
            ..configs::orderbook::Configuration::test_default()
        })
        .await;

    // Place order
    let order_ab = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 1u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let order_ac = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 1u64.eth(),
        buy_token: *token_c.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let order_ab_id = services.create_order(&order_ab).await.unwrap();
    let order_ac_id = services.create_order(&order_ac).await.unwrap();

    tracing::info!("Waiting for both orders to be in the auction");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let auction = services.get_auction().await.auction;
        auction.orders.len() == 2
    })
    .await
    .unwrap();

    // good solver settles order_ab at a price 3:1
    good_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (*token_a.address(), 3u64.eth()),
            (*token_b.address(), 1u64.eth()),
        ]),
        trades: vec![solvers_dto::solution::Trade::Fulfillment(
            solvers_dto::solution::Fulfillment {
                executed_amount: order_ab.sell_amount,
                fee: Some(::alloy::primitives::U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_ab_id.0),
            },
        )],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
        wrappers: vec![],
        gas_fee_override: None,
    }));

    // bad solver settles both orders at 2:1. Because it can't beat the
    // reference solution of order_a provided by the good solver this
    // solution will get filtered during the combinatorial auction.
    bad_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (*token_a.address(), 2u64.eth()),
            (*token_b.address(), 1u64.eth()),
            (*token_c.address(), 1u64.eth()),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order_ab.sell_amount,
                fee: Some(::alloy::primitives::U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_ab_id.0),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order_ac.sell_amount,
                fee: Some(::alloy::primitives::U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_ac_id.0),
            }),
        ],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
        wrappers: vec![],
        gas_fee_override: None,
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
        Some(&U256::ZERO)
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
    assert_eq!(good_solution.reference_score, Some(U256::ZERO));

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

/// Regression guard for the `order_is_actively_bid_on` check: an order that is
/// bid on **only** by a non-winning (filtered) solution must still count as
/// actively bid on and therefore be non-replaceable.
///
/// This pins the observable endpoint behaviour across the solver_competition
/// v1 -> v2 refactor. Both the old path (`load_latest_competitions`, which
/// flattened the orders of every solution in the JSON blob) and the new path
/// (`recent_solution_order_uids`, which reads `proposed_trade_executions`)
/// consider *all* solutions, not just winners. The assertions here never
/// reference the internal query, so they must hold identically before and
/// after the refactor.
///
/// The setup mirrors `store_filtered_solutions`: the good solver wins with a
/// solution for `order_win`, while the bad solver bundles `order_win` and
/// `order_loser` into a single solution that gets filtered out during the
/// combinatorial auction (it cannot beat the good solver's reference on
/// `order_win`). As a result `order_loser` is bid on exclusively by a
/// non-winning solution and is never settled. Auto-mining is disabled so
/// nothing settles and the auction state stays frozen while we probe the
/// replacement behaviour.
async fn cannot_replace_order_bid_on_by_non_winning_solution(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [good_solver_account, bad_solver_account] = onchain.make_solvers(100u64.eth()).await;
    let [trader] = onchain.make_accounts(100u64.eth()).await;
    let [token_a, token_b, token_c] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(300_000u64.eth(), 1_000u64.eth())
        .await;

    // Give the settlement contract a ton of the traded tokens so that the mocked
    // solver solutions can simply give money away to make the trade execute.
    token_b
        .mint(*onchain.contracts().gp_settlement.address(), 50u64.eth())
        .await;
    token_c
        .mint(*onchain.contracts().gp_settlement.address(), 50u64.eth())
        .await;

    // Fund the trader generously so the replacement order comfortably passes
    // balance/allowance validation while the original orders are still open.
    token_a.mint(trader.address(), 10u64.eth()).await;
    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;

    let good_solver = Mock::new().await;
    let bad_solver = Mock::new().await;

    let base_tokens = vec![*token_a.address(), *token_b.address(), *token_c.address()];
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                good_solver_account.clone(),
                *onchain.contracts().weth.address(),
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
                haircut_bps: 0,
                submission_keys: vec![],
            },
            SolverEngine {
                name: "bad_solver".into(),
                account: bad_solver_account.clone(),
                endpoint: bad_solver.url.clone(),
                base_tokens,
                merge_solutions: true,
                haircut_bps: 0,
                submission_keys: vec![],
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let config = Configuration::test_no_drivers();
    services
        .start_autopilot(
            None,
            Configuration {
                drivers: vec![
                    Solver::test("good_solver", good_solver_account.address()),
                    Solver::test("bad_solver", bad_solver_account.address()),
                ],
                order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                    "test_solver",
                    "http://localhost:11088/test_solver",
                )]),
                run_loop: RunLoopConfig {
                    max_winners_per_auction: std::num::NonZeroUsize::new(10).unwrap(),
                    ..config.run_loop
                },
                ..config
            },
        )
        .await;
    services
        .start_api(configs::orderbook::Configuration {
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "test_solver",
                "http://localhost:11088/test_solver",
            )]),
            ..configs::orderbook::Configuration::test_default()
        })
        .await;

    // The good solver wins this order.
    let order_win = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 1u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    // This order is only ever part of the bad solver's filtered solution.
    let order_loser = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 1u64.eth(),
        buy_token: *token_c.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let order_win_id = services.create_order(&order_win).await.unwrap();
    let order_loser_id = services.create_order(&order_loser).await.unwrap();

    // The good solver settles `order_win` at a favourable 3:1 price.
    good_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (*token_a.address(), 3u64.eth()),
            (*token_b.address(), 1u64.eth()),
        ]),
        trades: vec![solvers_dto::solution::Trade::Fulfillment(
            solvers_dto::solution::Fulfillment {
                executed_amount: order_win.sell_amount,
                fee: Some(U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_win_id.0),
            },
        )],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
        wrappers: vec![],
        gas_fee_override: None,
    }));

    // The bad solver bundles both orders at a worse 2:1 price. Because it can't
    // beat the good solver's reference on `order_win`, the whole solution
    // (including its `order_loser` trade) is filtered out.
    bad_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (*token_a.address(), 2u64.eth()),
            (*token_b.address(), 1u64.eth()),
            (*token_c.address(), 1u64.eth()),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order_win.sell_amount,
                fee: Some(U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_win_id.0),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order_loser.sell_amount,
                fee: Some(U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_loser_id.0),
            }),
        ],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
        wrappers: vec![],
        gas_fee_override: None,
    }));

    // Freeze the chain: no settlement is ever mined, so both orders stay open
    // and `order_loser` remains bid on (in the filtered solution) indefinitely.
    web3.provider.evm_set_automine(false).await.unwrap();

    // Drive auctions by hand until a competition is stored in which `order_loser`
    // appears in a non-winning solution. We use the internal (unfiltered)
    // endpoint because the public one hides competitions before their deadline.
    tracing::info!("waiting for order_loser to be bid on by a non-winning solution");
    let latest_auction_id = || async {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar::<_, i64>("SELECT id FROM competition_auctions ORDER BY id DESC LIMIT 1")
            .fetch_optional(&mut *db)
            .await
            .unwrap()
    };
    let loser_bid_on_by_non_winner = || async {
        onchain.mint_block().await;
        let Some(auction_id) = latest_auction_id().await else {
            return false;
        };
        match services.get_solver_competition_unfiltered(auction_id).await {
            Ok(competition) => competition.solutions.iter().any(|solution| {
                !solution.is_winner
                    && solution
                        .orders
                        .iter()
                        .any(|order| order.id == order_loser_id)
            }),
            Err(_) => false,
        }
    };
    wait_for_condition(TIMEOUT, loser_bid_on_by_non_winner)
        .await
        .unwrap();

    // Sanity checks on the scenario: `order_loser` is bid on, exclusively by
    // non-winning solutions, and was never executed.
    let auction_id = latest_auction_id().await.unwrap();
    let competition = services
        .get_solver_competition_unfiltered(auction_id)
        .await
        .unwrap();
    let loser_solutions: Vec<_> = competition
        .solutions
        .iter()
        .filter(|solution| {
            solution
                .orders
                .iter()
                .any(|order| order.id == order_loser_id)
        })
        .collect();
    assert!(
        !loser_solutions.is_empty(),
        "order_loser should have been bid on"
    );
    assert!(
        loser_solutions.iter().all(|solution| !solution.is_winner),
        "order_loser must only appear in non-winning solutions"
    );
    assert!(
        services
            .get_trades(&order_loser_id)
            .await
            .unwrap()
            .is_empty(),
        "order_loser must not have been executed"
    );

    // Attempt to replace `order_loser`. Even though it only appeared in a
    // losing solution, it counts as actively bid on and the replacement must be
    // rejected.
    let replacement = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 1u64.eth(),
        buy_token: *token_c.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: OrderCreationAppData::Full {
            full: format!(
                r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{order_loser_id}"}}}}}}"#
            ),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let (error_code, error_message) = services
        .create_order(&replacement)
        .await
        .expect_err("replacing an actively-bid-on order must be rejected");
    assert_eq!(error_code, StatusCode::BAD_REQUEST, "body: {error_message}");
    assert!(
        error_message.contains("OldOrderActivelyBidOn"),
        "expected OldOrderActivelyBidOn, got: {error_message}"
    );
}
