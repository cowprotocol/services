use crate::tests::{
    self,
    setup::{ab_order, ab_pool, ab_solution},
};

/// Verify that a solution with custom gas fee overrides settles successfully.
#[tokio::test]
#[ignore]
async fn settle_with_gas_fee_override() {
    let test = tests::setup()
        .name("gas fee override")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().gas_fee_override(100_000_000_000, 2_000_000_000))
        .done()
        .await;

    let id = test.solve().await.ok().id();
    test.settle(id)
        .await
        .ok()
        .await
        .ab_order_executed(&test)
        .await;
}
