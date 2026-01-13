//! Tests for the haircut feature which applies conservative bidding by reducing
//! solver-reported economics.

use crate::{
    domain::competition::order,
    tests::{
        self,
        cases::EtherExt,
        setup::{ab_order, ab_pool, ab_solution},
    },
};

/// Test that haircut correctly reduces the solution score for orders in real
/// auctions. The haircut adjusts clearing prices to report lower output
/// amounts, making the bid more conservative.
#[tokio::test]
#[ignore]
async fn order_haircut_reduces_score() {
    // Use a limit order with enough slack for haircut
    // The pool has 100000:6000 ratio, so selling 50 A gets ~2.97 B
    // We set a generous buy_amount limit (e.g., 2 B) to create slack
    let side = order::Side::Sell;
    let kind = order::Kind::Limit;

    // First, get baseline without haircut
    let test_no_haircut = tests::setup()
        .name("Order haircut - baseline (0 bps)")
        .pool(ab_pool())
        .order(
            ab_order()
                .side(side)
                .kind(kind)
                .buy_amount(2u64.ether().into_wei()) // Low limit creates slack
                .solver_fee(Some("1e-16".ether().into_wei())),
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(0)])
        .done()
        .await;

    let solve_no_haircut = test_no_haircut.solve().await.ok();
    let score_no_haircut = solve_no_haircut.score();

    // Now test with 500 bps (5%) haircut
    let test_with_haircut = tests::setup()
        .name("Order haircut - with 500 bps (5%)")
        .pool(ab_pool())
        .order(
            ab_order()
                .side(side)
                .kind(kind)
                .buy_amount(2u64.ether().into_wei()) // Same low limit
                .solver_fee(Some("1e-16".ether().into_wei())),
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(500)])
        .done()
        .await;

    let solve_with_haircut = test_with_haircut.solve().await.ok();
    let score_with_haircut = solve_with_haircut.score();

    tracing::info!(
        %score_no_haircut,
        %score_with_haircut,
        "Comparing scores with and without haircut"
    );

    // The haircutted solution should have a lower score because the adjusted
    // clearing prices imply less surplus
    assert!(
        score_with_haircut < score_no_haircut,
        "Haircut should reduce solution score: {} >= {}",
        score_with_haircut,
        score_no_haircut
    );
}

/// Test that haircut is properly applied for buy orders.
#[tokio::test]
#[ignore]
async fn buy_order_haircut() {
    let side = order::Side::Buy;
    let kind = order::Kind::Limit;

    // For buy orders, haircut reduces the buy amount received
    let test_no_haircut = tests::setup()
        .name("Buy order haircut - baseline")
        .pool(ab_pool())
        .order(
            ab_order()
                .side(side)
                .kind(kind)
                .sell_amount(100u64.ether().into_wei()) // Generous sell limit
                .solver_fee(Some("1e-16".ether().into_wei())),
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(0)])
        .done()
        .await;

    let solve_no_haircut = test_no_haircut.solve().await.ok();
    let score_no_haircut = solve_no_haircut.score();

    let test_with_haircut = tests::setup()
        .name("Buy order haircut - with 500 bps")
        .pool(ab_pool())
        .order(
            ab_order()
                .side(side)
                .kind(kind)
                .sell_amount(100u64.ether().into_wei())
                .solver_fee(Some("1e-16".ether().into_wei())),
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(500)])
        .done()
        .await;

    let solve_with_haircut = test_with_haircut.solve().await.ok();
    let score_with_haircut = solve_with_haircut.score();

    tracing::info!(
        %score_no_haircut,
        %score_with_haircut,
        "Comparing buy order scores with and without haircut"
    );

    // Haircut should reduce the score
    assert!(
        score_with_haircut < score_no_haircut,
        "Haircut should reduce buy order score: {} >= {}",
        score_with_haircut,
        score_no_haircut
    );
}
