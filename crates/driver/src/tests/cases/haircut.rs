//! Tests for the haircut feature which applies conservative bidding by reducing
//! solver-reported economics.

use {
    crate::{
        domain::{competition::order, eth},
        tests::{
            self,
            cases::EtherExt,
            setup::{ab_order, ab_pool, ab_solution},
        },
    },
    number::units::EthUnit,
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
                .buy_amount(2u64.eth()) // Low limit creates surplus
                .solver_fee(Some(eth::U256::from(100))),
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
                .solver_fee(Some(eth::U256::from(100))),
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(500)])
        .done()
        .await;

    let solve_with_haircut = test_with_haircut.solve().await.ok();
    let score_with_haircut = solve_with_haircut.score();

    // With 500 bps (5%) haircut, the score should be reduced by approximately 5%.
    // Compute the actual percentage: (score_with_haircut * 100) / score_no_haircut
    // Should be approximately 95 (allowing 94-96 range for tolerance).
    let percentage: u64 = ((score_with_haircut * eth::U256::from(100)) / score_no_haircut)
        .try_into()
        .unwrap();

    assert!(
        (94..=96).contains(&percentage),
        "Haircut score {} should be ~95% of baseline {}, but was {}%",
        score_with_haircut,
        score_no_haircut,
        percentage
    );
}

/// Test that haircut is properly applied for buy orders.
/// For buy orders, the haircut reduces the effective buy amount, which
/// increases the sell amount the user pays. This reduces surplus and thus the
/// score. Note: The percentage reduction for buy orders differs from sell
/// orders because the haircut is applied to the executed buy amount, not
/// directly to surplus.
#[tokio::test]
#[ignore]
async fn buy_order_haircut() {
    let side = order::Side::Buy;
    let kind = order::Kind::Limit;

    // For buy orders, we need to set a buy_amount that creates enough surplus.
    // The pool has 100000:6000 ratio. For a buy order wanting 2.97 B,
    // we'd need to sell ~50 A. Setting a generous sell limit creates surplus.
    let test_no_haircut = tests::setup()
        .name("Buy order haircut - baseline")
        .pool(ab_pool())
        .order(
            ab_order()
                .side(side)
                .kind(kind)
                .buy_amount(2u64.eth()) // Target buy amount
                .sell_amount(100u64.ether().into_wei()) // Generous sell limit creates surplus
                .solver_fee(Some(eth::U256::from(100))),
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
                .buy_amount(2u64.eth()) // Same target buy amount
                .sell_amount(100u64.ether().into_wei()) // Same generous sell limit
                .solver_fee(Some(eth::U256::from(100))),
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(500)])
        .done()
        .await;

    let solve_with_haircut = test_with_haircut.solve().await.ok();
    let score_with_haircut = solve_with_haircut.score();

    // For buy orders, the haircut is applied to the executed buy amount and then
    // converted to sell token. The impact on score depends on the price ratio.
    // With 500 bps (5%) haircut on a 2 ETH buy amount, the haircut is 0.1 ETH in
    // buy token. When converted to sell token at the pool's price ratio, this
    // results in a smaller percentage impact on the score compared to sell
    // orders. Expected: score reduction of ~1% (percentage ~99%) rather than
    // 5%.
    let percentage: u64 = ((score_with_haircut * eth::U256::from(100)) / score_no_haircut)
        .try_into()
        .unwrap();

    // For buy orders with this setup, expect ~99% (1% reduction) due to price
    // conversion
    assert!(
        (98..=100).contains(&percentage) && score_with_haircut < score_no_haircut,
        "Haircut score {} should be ~99% of baseline {} (reduced by ~1%), but was {}%",
        score_with_haircut,
        score_no_haircut,
        percentage
    );
}
