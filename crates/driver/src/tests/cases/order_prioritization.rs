use crate::tests::setup::{ab_order, ab_pool, ab_solution, setup, Order};

/// Test that orders are sorted correctly before being sent to the solver:
/// market orders come before limit orders, and orders that are more likely to
/// fulfill come before orders that are less likely (according to token prices
/// in ETH).
#[tokio::test]
#[ignore]
async fn sorting() {
    let test = setup()
        .pool(ab_pool())
        // Orders with better price ratios come first.
        .order(ab_order())
        .order(ab_order().reduce_amount(1000000000000000u128.into()).rename("second order"))
        // Limit orders come after market orders.
        .order(
            ab_order()
                .rename("third order")
                .limit()
        )
        .order(ab_order().reduce_amount(1000000000000000u128.into()).rename("fourth order").limit())
        .solution(ab_solution())
        .done()
        .await;

    // Only check that the solve endpoint can be called successfully, which means
    // that the solver received the orders sorted.
    test.solve().await.ok();
}

/// If a user does not have enough tokens to settle all their orders filter out
/// the least likely to settle ones that go over the user's budget.
#[tokio::test]
#[ignore]
async fn filtering() {
    let test = setup()
        .pool(ab_pool())
        // Orders with better price ratios come first.
        .order(ab_order())
        .order(ab_order().reduce_amount(1000000000000000u128.into()).rename("second order"))
        // Filter out the next order, because the trader doesn't have enough balance to cover it.
        .order(
            ab_order()
                .rename("third order")
                .multiply_amount(100000000000000000u128.into())
                .filtered()
        )
        // Filter out the next order. It can't be fulfilled due to the balance that is required to
        // fulfill the previous orders.
        .order(
            Order {
                sell_amount: 4999999999900002000000000000000u128.into(),
                surplus_factor: 1.into(),
                ..ab_order()
            }
            .rename("fourth order")
            .unfunded()
            .filtered()
            .limit()
        )
        .solution(ab_solution())
        .done()
        .await;

    // Only check that the solve endpoint can be called successfully, which means
    // that the solver received the orders sorted.
    test.solve().await.ok();
}
