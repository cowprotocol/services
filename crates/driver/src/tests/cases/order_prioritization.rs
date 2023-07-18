use crate::{
    domain::{competition::order, eth},
    tests::{
        cases::AB_ORDER_AMOUNT,
        setup::{ab_order, ab_pool, ab_solution, setup, Order},
    },
};

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
        .order(
            ab_order()
                .reduce_amount(1000000000000000u128.into()),
        )
        .order(ab_order().rename("second order"))
        // Limit orders come after market orders.
        .order(
            ab_order()
                .rename("third order")
                .limit()
                .reduce_amount(1000000000000000u128.into()),
        )
        .order(ab_order().rename("fourth order").limit())
        .solution(ab_solution())
        .done()
        .await;

    // Only check that the solve endpoint can be called successfully, which means
    // that the solver received the orders sorted.
    test.solve().await.ok();
}

// TODO Comment
#[tokio::test]
#[ignore]
async fn filtering() {
    let test = setup()
        .pool(ab_pool())
        // Orders with better price ratios come first.
        .order(
            ab_order()
                .reduce_amount(1000000000000000u128.into()),
        )
        .order(ab_order().rename("second order"))
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
                sell_amount: 4999899999900002000000000000000u128.into(),
                surplus_factor: 1.into(),
                ..ab_order()
            }
            .rename("fourth order")
            .unfunded()
            .filtered()
            .limit()
        )
        // This order sells B and buys A, while all the previous orders sold A and bought B. The
        // trader isn't funded the tokens to cover this order, but because of the previous orders
        // which buy B, he will have enough balance to cover it. Hence, it doesn't get filtered
        // out.
        .order(
            Order {
                name: "fifth order",
                sell_amount: 100000000.into(),
                sell_token: "B",
                buy_token: "A",
                ..Default::default()
            }
            // Don't fund the trader to cover this order, instead rely on the previous orders.
            .unfunded()
            .limit()
        )
        .solution(ab_solution())
        .done()
        .await;

    // Only check that the solve endpoint can be called successfully, which means
    // that the solver received the orders sorted.
    test.solve().await.ok();
}
