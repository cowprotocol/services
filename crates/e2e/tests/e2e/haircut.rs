//! Tests for the haircut feature which applies conservative bidding by reducing
//! solver-reported economics. This specifically tests that settlements with
//! haircut are correctly indexed by the autopilot.
//!
//! The haircut feature was initially broken because:
//! 1. On-chain `executed_amount = executed + fee` (no haircut)
//! 2. Reported `executed_sell = executed + fee + haircut` (includes haircut)
//! 3. Autopilot comparison failed: reported > on-chain → `SolutionNotFound`
//!
//! The fix incorporates haircut into the fee/executed amounts for settlements,
//! so on-chain matches reported amounts.

use {
    e2e::setup::{colocation::SolverEngine, *},
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::ethrpc::Web3,
};

/// Test that a settlement with haircut is correctly indexed by the autopilot.
///
/// This test verifies that when a solver applies a haircut to their solution,
/// the autopilot can still match the on-chain settlement with the stored
/// solution. Previously, haircut caused a mismatch because it was stored
/// separately and added to `sell_amount()` only for reporting, but didn't
/// affect the actual executed amounts.
///
/// With the fix, haircut is incorporated into the fee for settlements (orders
/// with dynamic fees), so the reported amounts match the on-chain amounts.
#[tokio::test]
#[ignore]
async fn local_node_settlement_with_haircut_is_indexed() {
    run_test(settlement_with_haircut_is_indexed).await;
}

async fn settlement_with_haircut_is_indexed(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader
    token_a.mint(trader.address(), 10u64.eth()).await;

    // Approve GPv2 for trading
    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Start system with a solver that has 200 bps (2%) haircut
    let haircut_bps = 200u32;
    colocation::start_driver(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver.clone(),
            endpoint: colocation::start_baseline_solver_with_haircut(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                haircut_bps,
            )
            .await
            .endpoint,
            base_tokens: vec![],
            merge_solutions: true,
            haircut_bps,
        }],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=test_solver|http://localhost:11088/test_solver|{}",
                    const_hex::encode(solver.address())
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Place a sell order
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

    // Wait for the settlement to be indexed
    // This is the critical part - if haircut handling is broken, the autopilot
    // will fail to match the on-chain trade with the stored solution and return
    // `SolutionNotFound` error.
    tracing::info!("waiting for settlement to be indexed");
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

    // Verify the settlement was indexed correctly
    let trades = services.get_trades(&uid).await.unwrap();
    assert!(!trades.is_empty(), "Trade should exist");

    let competition = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .expect("Settlement should be indexed - SolutionNotFound would indicate haircut bug");

    // Verify the solution is marked as winner
    assert!(
        !competition.solutions.is_empty(),
        "Should have at least one solution"
    );
    let winner = competition
        .solutions
        .iter()
        .find(|s| s.is_winner)
        .expect("Should have a winning solution");
    assert_eq!(
        winner.solver_address,
        solver.address(),
        "Winner should be our solver"
    );

    // Verify the order shows executed amounts
    let order_details = services.get_order(&uid).await.unwrap();
    assert!(
        order_details.metadata.executed_sell_amount > 0u64.into(),
        "Order should have non-zero executed sell amount"
    );
    assert!(
        order_details.metadata.executed_buy_amount > 0u64.into(),
        "Order should have non-zero executed buy amount"
    );
}

/// Test that a buy order settlement with haircut is correctly indexed.
///
/// Buy orders have different haircut handling - the haircut is applied to the
/// buy amount and then converted to sell tokens. This test verifies that this
/// conversion doesn't break settlement indexing.
#[tokio::test]
#[ignore]
async fn local_node_buy_order_settlement_with_haircut_is_indexed() {
    run_test(buy_order_settlement_with_haircut_is_indexed).await;
}

async fn buy_order_settlement_with_haircut_is_indexed(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader with enough to cover the buy order
    token_a.mint(trader.address(), 100u64.eth()).await;

    // Approve GPv2 for trading
    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Start system with a solver that has 500 bps (5%) haircut
    let haircut_bps = 500u32;
    colocation::start_driver(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver.clone(),
            endpoint: colocation::start_baseline_solver_with_haircut(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
                haircut_bps,
            )
            .await
            .endpoint,
            base_tokens: vec![],
            merge_solutions: true,
            haircut_bps,
        }],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=test_solver|http://localhost:11088/test_solver|{}",
                    const_hex::encode(solver.address())
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Place a buy order - want to buy 5 WETH, willing to sell up to 50 token_a
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 50u64.eth(), // Maximum willing to sell
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 5u64.eth(), // Amount we want to buy
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
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
        let balance = token_a.balanceOf(trader.address()).call().await.unwrap();
        // Balance should decrease (some tokens sold)
        balance < 100u64.eth()
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Wait for the settlement to be indexed
    tracing::info!("waiting for settlement to be indexed");
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

    // Verify the settlement was indexed correctly
    let trades = services.get_trades(&uid).await.unwrap();
    assert!(!trades.is_empty(), "Trade should exist");

    let competition = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .expect(
            "Buy order settlement should be indexed - SolutionNotFound would indicate haircut bug",
        );

    // Verify the solution is marked as winner
    assert!(
        !competition.solutions.is_empty(),
        "Should have at least one solution"
    );
    let winner = competition
        .solutions
        .iter()
        .find(|s| s.is_winner)
        .expect("Should have a winning solution");
    assert_eq!(
        winner.solver_address,
        solver.address(),
        "Winner should be our solver"
    );
}
