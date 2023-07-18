use crate::tests::{
    setup,
    setup::{ab_order, ab_pool, ab_solution},
};

/// Test that orders are sorted correctly before being sent to the solver:
/// market orders come before limit orders, and orders that are more likely to
/// fulfill come before orders that are less likely (according to token prices
/// in ETH).
#[tokio::test]
#[ignore]
async fn test() {
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
