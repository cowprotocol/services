//! Tests for the haircut feature which applies conservative bidding by reducing
//! solver-reported economics.

use {
    crate::{
        domain::competition::order,
        tests::{
            self,
            cases::EtherExt,
            setup::{ab_order, ab_pool, ab_solution},
        },
    },
    number::{testing::ApproxEq, units::EthUnit},
    shared::domain::eth,
};

/// Haircut in basis points used across tests (500 bps = 5%)
const HAIRCUT_BPS: u32 = 500;

/// Test that haircut correctly reduces the solution score for sell orders.
/// The haircut reduces the reported buy_amount, making the bid more
/// conservative.
///
/// Verifies that:
/// - `executedSell == signedSellAmount` (fill-or-kill requires exact execution)
/// - `executedBuy` with haircut < `executedBuy` without haircut (haircut
///   reduces output)
#[tokio::test]
#[ignore]
async fn order_haircut_reduces_score() {
    // Use a limit order with enough slack for haircut
    // The pool has 100000:6000 ratio, so selling 50 A gets ~2.97 B
    // We set a generous buy_amount limit (e.g., 2 B) to create slack
    let side = order::Side::Sell;
    let kind = order::Kind::Limit;
    let signed_sell_amount = ab_order().sell_amount;

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
        .solvers(vec![tests::setup::test_solver().haircut_bps(HAIRCUT_BPS)])
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

    // Extract executedBuy from baseline (no haircut)
    let solution_no_haircut = solve_no_haircut.solution();
    let orders_no_haircut = solution_no_haircut
        .get("orders")
        .unwrap()
        .as_object()
        .unwrap();
    let executed_buy_no_haircut = orders_no_haircut
        .values()
        .next()
        .unwrap()
        .get("executedBuy")
        .and_then(|v| v.as_str())
        .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
        .unwrap();

    // Verify that reported sell amount matches signed amount exactly.
    // Fill-or-kill orders require exact execution.
    let solution = solve_with_haircut.solution();
    let orders = solution.get("orders").unwrap().as_object().unwrap();
    for (_uid, order_data) in orders {
        let executed_sell = order_data
            .get("executedSell")
            .and_then(|v| v.as_str())
            .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
            .unwrap();
        let executed_buy = order_data
            .get("executedBuy")
            .and_then(|v| v.as_str())
            .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
            .unwrap();
        let limit_sell = order_data
            .get("limitSell")
            .and_then(|v| v.as_str())
            .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
            .unwrap();

        assert!(
            executed_sell == signed_sell_amount,
            "Sell order: executedSell {} does not match signed sell amount {} (fill-or-kill \
             requires exact execution)",
            executed_sell,
            signed_sell_amount
        );
        assert!(
            executed_sell <= limit_sell,
            "executedSell {} exceeds limitSell {}",
            executed_sell,
            limit_sell
        );

        // Verify haircut reduces executedBuy for sell orders by approximately
        // HAIRCUT_BPS
        let expected_buy =
            executed_buy_no_haircut * eth::U256::from(10000 - HAIRCUT_BPS) / eth::U256::from(10000);
        assert!(
            executed_buy.is_approx_eq(&expected_buy, Some(0.01)),
            "Sell order: executedBuy {} should be ~{}% of baseline {} (expected ~{})",
            executed_buy,
            100 - HAIRCUT_BPS / 100,
            executed_buy_no_haircut,
            expected_buy
        );
    }
}

/// Test that haircut is properly applied for buy orders.
/// For buy orders, the haircut increases the sell_amount the user pays.
/// This reduces surplus and thus the score.
///
/// Verifies that:
/// - `executedBuy == signedBuyAmount` (fill-or-kill must execute exactly)
/// - `executedSell <= sellLimit` (haircut increases sell, but must stay within
///   limit)
/// - `executedSell` with haircut > `executedSell` without haircut (haircut
///   increases cost)
#[tokio::test]
#[ignore]
async fn buy_order_haircut() {
    let side = order::Side::Buy;
    let kind = order::Kind::Limit;
    let signed_buy_amount = 2u64.eth();
    let signed_sell_limit = 100u64.ether().into_wei();

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
                .buy_amount(signed_buy_amount) // Target buy amount (what user signs for)
                .sell_amount(signed_sell_limit) // Generous sell limit creates surplus
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
                .buy_amount(signed_buy_amount) // Same target buy amount
                .sell_amount(signed_sell_limit) // Same generous sell limit
                .solver_fee(Some(eth::U256::from(100))),
        )
        .solution(ab_solution())
        .solvers(vec![tests::setup::test_solver().haircut_bps(HAIRCUT_BPS)])
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

    // Extract executedSell from baseline (no haircut)
    let solution_no_haircut = solve_no_haircut.solution();
    let orders_no_haircut = solution_no_haircut
        .get("orders")
        .unwrap()
        .as_object()
        .unwrap();
    let executed_sell_no_haircut = orders_no_haircut
        .values()
        .next()
        .unwrap()
        .get("executedSell")
        .and_then(|v| v.as_str())
        .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
        .unwrap();

    // Verify buy order constraints:
    // - Fill-or-kill must execute exactly (executedBuy == signedBuyAmount)
    // - Don't take more than user's maximum (executedSell <= sellLimit)
    let solution = solve_with_haircut.solution();
    let orders = solution.get("orders").unwrap().as_object().unwrap();
    for (_uid, order_data) in orders {
        let executed_sell = order_data
            .get("executedSell")
            .and_then(|v| v.as_str())
            .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
            .unwrap();
        let executed_buy = order_data
            .get("executedBuy")
            .and_then(|v| v.as_str())
            .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
            .unwrap();
        let limit_sell = order_data
            .get("limitSell")
            .and_then(|v| v.as_str())
            .and_then(|s| eth::U256::from_str_radix(s, 10).ok())
            .unwrap();

        assert!(
            executed_buy == signed_buy_amount,
            "Buy order: executedBuy {} does not match signed buy amount {} (fill-or-kill requires \
             exact execution)",
            executed_buy,
            signed_buy_amount
        );
        assert!(
            executed_sell <= signed_sell_limit,
            "Buy order: executedSell {} exceeds sell limit {}. Haircut increases sell_amount but \
             it must still respect the user's limit!",
            executed_sell,
            signed_sell_limit
        );
        assert!(
            executed_sell <= limit_sell,
            "executedSell {} exceeds limitSell {}",
            executed_sell,
            limit_sell
        );

        // Verify haircut increases executedSell for buy orders.
        // For buy orders, haircut increases the sell amount proportionally.
        let haircut_ratio = 1.0 + (HAIRCUT_BPS as f64 / 10000.0); // ~1.05 for 500 bps
        let expected_sell =
            eth::U256::from((executed_sell_no_haircut.to::<u128>() as f64 * haircut_ratio) as u128);
        assert!(
            executed_sell.is_approx_eq(&expected_sell, Some(0.02)),
            "Buy order: executedSell {} should be ~{:.1}% higher than baseline {} (expected ~{})",
            executed_sell,
            (haircut_ratio - 1.0) * 100.0,
            executed_sell_no_haircut,
            expected_sell
        );
    }
}
