//! Parameterized end-to-end tests for the haircut "make-room" pre-processing.
//!
//! The driver tightens each order's limits before sending the auction to the
//! solver, so that any bid the solver returns at the tightened limit still
//! respects the user's signed limit price after the post-hoc haircut is
//! applied. These tests prove that round-trip closes: solver bids exactly at
//! the tightened limit it sees, driver applies the haircut, and the reported
//! amounts land on the user's signed limit.
//!
//! Modelled on the `volume_protocol_fee_*_at_limit_price` cases in
//! [`super::protocol_fees`], which are the closest existing precedent — there
//! too, the driver post-processes the solver's bid and the test asserts that
//! the user lands on the signed limit.

use {
    crate::{
        domain::competition::order,
        tests::{
            self,
            cases::EtherExt,
            setup::{
                ExpectedOrderAmounts,
                Test,
                ab_adjusted_pool,
                ab_liquidity_quote,
                ab_order,
                ab_solution,
                test_solver,
            },
        },
    },
    eth_domain_types as eth,
};

struct Amounts {
    sell: eth::U256,
    buy: eth::U256,
}

struct Execution {
    // What the solver bids against the tightened limit it sees.
    solver: Amounts,
    // What the driver reports after applying the haircut.
    driver: Amounts,
}

struct Order {
    sell_amount: eth::U256,
    buy_amount: eth::U256,
    side: order::Side,
}

struct TestCase {
    order: Order,
    haircut_bps: u32,
    execution: Execution,
    expected_score: eth::U256,
    partial: bool,
}

async fn run(test_case: TestCase) {
    let test_name = format!(
        "Haircut make-room: {:?} {} bps{}",
        test_case.order.side,
        test_case.haircut_bps,
        if test_case.partial { " partial" } else { "" },
    );
    let quote = ab_liquidity_quote()
        .sell_amount(test_case.execution.solver.sell)
        .buy_amount(test_case.execution.solver.buy);
    let pool = ab_adjusted_pool(quote);
    // Use a tiny constant network fee. The at-limit haircut math is sensitive
    // to a percent-based `solver_fee` (the haircut conversion through clearing
    // prices doesn't absorb it the way the volume-fee path does), and we still
    // need a non-zero fee to keep these orders out of the StaticFee path.
    let solver_fee = eth::U256::from(100);
    let executed = match test_case.order.side {
        order::Side::Buy => (test_case.order.buy_amount > test_case.execution.solver.buy)
            .then_some(test_case.execution.solver.buy),
        order::Side::Sell => (test_case.order.sell_amount > test_case.execution.solver.sell)
            .then_some(test_case.execution.solver.sell - solver_fee),
    };
    let expected_amounts = ExpectedOrderAmounts {
        sell: test_case.execution.driver.sell,
        buy: test_case.execution.driver.buy,
    };

    let mut order = ab_order()
        .kind(order::Kind::Limit)
        .sell_amount(test_case.order.sell_amount)
        .buy_amount(test_case.order.buy_amount)
        .solver_fee(Some(solver_fee))
        .side(test_case.order.side)
        .executed(executed)
        .no_surplus()
        .expected_amounts(expected_amounts);
    if test_case.partial {
        order = order.partial(eth::U256::ZERO);
    }

    let test: Test = tests::setup()
        .name(test_name)
        .pool(pool)
        .order(order.clone())
        .solution(ab_solution())
        .solvers(vec![test_solver().haircut_bps(test_case.haircut_bps)])
        .done()
        .await;

    let result = test.solve().await.ok();
    // At the limit price the make-room math closes to zero surplus over the
    // signed limit, but the integer-price encoding leaves a few wei of noise.
    // `is_approx_eq` would divide-by-zero against `expected_score = 0`, so
    // compare with an absolute wei tolerance instead.
    let score = result.score();
    let diff = if score > test_case.expected_score {
        score - test_case.expected_score
    } else {
        test_case.expected_score - score
    };
    assert!(
        diff <= eth::U256::from(1_000_000u64),
        "score {} differs from expected {} by {} wei",
        score,
        test_case.expected_score,
        diff,
    );
    result.orders(&[order]);
}

/// Sell order: solver bids at the tightened buy limit (40 / (1 - 0.2) = 50)
/// and the driver's 2000 bps haircut brings the reported buy back to the
/// user's signed limit of 40 ETH. Mirrors
/// [`super::protocol_fees::volume_protocol_fee_sell_order_at_limit_price`].
#[tokio::test]
#[ignore]
async fn sell_order_at_limit_price() {
    let test_case = TestCase {
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Sell,
        },
        haircut_bps: 2000,
        execution: Execution {
            // Solver clears at the tightened limit (40 / (1 - 0.2) = 50 buy).
            solver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 50.ether().into_wei(),
            },
            // Driver subtracts the 20% haircut, landing exactly on the signed limit.
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: eth::U256::ZERO,
        partial: false,
    };
    run(test_case).await;
}

/// Buy order: solver bids at the tightened sell limit (50 / (1 + 0.25) = 40)
/// and the driver's 2500 bps haircut adds back exactly the amount needed for
/// the reported sell to equal the user's signed limit of 50 ETH. Mirrors
/// [`super::protocol_fees::volume_protocol_fee_buy_order_at_limit_price`].
#[tokio::test]
#[ignore]
async fn buy_order_at_limit_price() {
    let test_case = TestCase {
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 40.ether().into_wei(),
            side: order::Side::Buy,
        },
        haircut_bps: 2500,
        execution: Execution {
            // Solver clears at the tightened limit (50 / (1 + 0.25) = 40 sell).
            solver: Amounts {
                sell: 40.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
            // Driver adds the 25% haircut, landing exactly on the signed limit.
            driver: Amounts {
                sell: 50.ether().into_wei(),
                buy: 40.ether().into_wei(),
            },
        },
        expected_score: eth::U256::ZERO,
        partial: false,
    };
    run(test_case).await;
}

/// Partial sell order, scaled-down version of the at-limit case. Mirrors
/// [`super::protocol_fees::volume_protocol_fee_partial_sell_order_at_limit_price`].
#[tokio::test]
#[ignore]
async fn partial_sell_order_at_limit_price() {
    let test_case = TestCase {
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Sell,
        },
        haircut_bps: 2000,
        execution: Execution {
            // 40% partial fill at the tightened limit (20 / (1 - 0.2) = 25 buy).
            solver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 25.ether().into_wei(),
            },
            // 20% haircut on the partial buy lands on the partial signed limit.
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: eth::U256::ZERO,
        partial: true,
    };
    run(test_case).await;
}

/// Partial buy order, scaled-down version of the at-limit case. Mirrors
/// [`super::protocol_fees::volume_protocol_fee_partial_buy_order_at_limit_price`].
#[tokio::test]
#[ignore]
async fn partial_buy_order_at_limit_price() {
    let test_case = TestCase {
        order: Order {
            sell_amount: 50.ether().into_wei(),
            buy_amount: 50.ether().into_wei(),
            side: order::Side::Buy,
        },
        haircut_bps: 2500,
        execution: Execution {
            // 40% partial fill at the tightened limit (20 / (1 + 0.25) = 16 sell).
            solver: Amounts {
                sell: 16.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
            // 25% haircut on the partial sell lands on the partial signed limit.
            driver: Amounts {
                sell: 20.ether().into_wei(),
                buy: 20.ether().into_wei(),
            },
        },
        expected_score: eth::U256::ZERO,
        partial: true,
    };
    run(test_case).await;
}
