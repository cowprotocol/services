use crate::{
    domain::competition::order,
    tests::{
        self,
        cases::DEFAULT_SURPLUS_FEE,
        setup::{ab_order, ab_pool, ab_solution, ExecutionDiff, Order, Solution},
    },
};

/// Run a matrix of tests for all meaningful combinations of order kind, side
/// and execution diff which fail asset flow verification.
#[tokio::test]
#[ignore]
async fn matrix() {
    for diff in [
        ExecutionDiff::decrease_buy(),
        ExecutionDiff::increase_sell(),
    ] {
        for side in [order::Side::Buy, order::Side::Sell] {
            for kind in [
                order::Kind::Market,
                order::Kind::Limit {
                    surplus_fee: order::SellAmount(DEFAULT_SURPLUS_FEE.into()),
                },
            ] {
                let test = tests::setup()
                    .name(format!("{side:?} {kind:?}\n{diff:?}"))
                    .pool(ab_pool())
                    .order(ab_order().side(side).kind(kind).execution_diff(diff))
                    .solution(ab_solution())
                    .done()
                    .await;

                // TODO When we add metrics, assert that an invalid asset flow error is traced.
                test.solve().await.err().kind("SolutionNotFound");
            }
        }
    }
}

/// Test that asset flow verification passes when sums of flows add up to zero.
#[tokio::test]
#[ignore]
async fn zero_sum() {
    let test = tests::setup()
        .disable_simulation()
        .pool(ab_pool())
        .order(
            ab_order()
                .rename("first order")
                .execution_diff(ExecutionDiff {
                    // This value is equal to the one below, ensuring a negative sum.
                    decrease_buy: 50.into(),
                    ..Default::default()
                }),
        )
        .order(
            ab_order()
                .rename("second order")
                .execution_diff(ExecutionDiff {
                    increase_buy: 50.into(),
                    ..Default::default()
                }),
        )
        .solution(Solution {
            orders: vec!["first order", "second order"],
            ..Default::default()
        })
        .done()
        .await;

    test.solve().await.ok();
}

/// Test that asset flow verification fails when sums of flows are negative.
#[tokio::test]
#[ignore]
async fn negative_sum() {
    let test = tests::setup()
        .disable_simulation()
        .pool(ab_pool())
        .order(
            ab_order()
                .rename("first order")
                .execution_diff(ExecutionDiff {
                    // This value is higher than the one below, ensuring a negative sum.
                    decrease_buy: 60.into(),
                    ..Default::default()
                }),
        )
        .order(
            ab_order()
                .rename("second order")
                .execution_diff(ExecutionDiff {
                    increase_buy: 50.into(),
                    ..Default::default()
                }),
        )
        .solution(Solution {
            orders: vec!["first order", "second order"],
            ..Default::default()
        })
        .done()
        .await;

    // TODO When we add metrics, assert that an invalid asset flow error is traced.
    test.solve().await.err().kind("SolutionNotFound");
}

/// Test that asset flow verification passes when sums of flows are positive.
#[tokio::test]
#[ignore]
async fn positive_sum() {
    let test = tests::setup()
        .disable_simulation()
        .pool(ab_pool())
        .order(
            ab_order()
                .rename("first order")
                .execution_diff(ExecutionDiff {
                    // This value is lower than the one below, ensuring a positive sum.
                    decrease_buy: 40.into(),
                    ..Default::default()
                }),
        )
        .order(
            ab_order()
                .rename("second order")
                .execution_diff(ExecutionDiff {
                    increase_buy: 50.into(),
                    ..Default::default()
                }),
        )
        .solution(Solution {
            orders: vec!["first order", "second order"],
            ..Default::default()
        })
        .done()
        .await;

    test.solve().await.ok();
}

/// Test that asset flow verification passes, even in the case where market,
/// limit, and partial liquidity buy/sell orders are mixed.
#[tokio::test]
#[ignore]
async fn mix() {
    let test = tests::setup()
        .disable_simulation()
        .pool(ab_pool())
        .order(
            ab_order()
                .rename("market order")
                .execution_diff(ExecutionDiff {
                    decrease_buy: 50.into(),
                    ..Default::default()
                }),
        )
        .order(
            ab_order()
                .rename("limit order")
                .limit()
                .execution_diff(ExecutionDiff {
                    increase_buy: 30.into(),
                    ..Default::default()
                })
                // Change the order UID by increasing valid_to. Otherwise, this order UID would be
                // the same as the one above.
                .increase_valid_to(),
        )
        .order(Order {
            name: "partial liquidity order",
            sell_amount: 50.into(),
            sell_token: "A",
            buy_token: "B",
            side: order::Side::Buy,
            partial: order::Partial::Yes {
                executed: Default::default(),
            },
            kind: order::Kind::Liquidity,
            execution_diff: ExecutionDiff {
                // This 20 plus the 30 above give 50, which equals to the first order, summing to
                // zero.
                increase_buy: 20.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .solution(Solution {
            orders: vec!["market order", "limit order", "partial liquidity order"],
            ..Default::default()
        })
        .done()
        .await;

    test.solve().await.ok();
}
