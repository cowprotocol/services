use crate::tests::{
    setup,
    setup::{ab_order, ab_pool, ab_solution, cd_order, cd_pool, cd_solution, Solution},
};

/// Test that settlements can be merged.
#[tokio::test]
#[ignore]
async fn possible() {
    let test = setup()
        .pool(ab_pool())
        .pool(cd_pool())
        .order(ab_order())
        .order(cd_order())
        .solution(ab_solution())
        .solution(cd_solution())
        .done()
        .await;

    let id = test
        .solve()
        .await
        .ok()
        .orders(&[ab_order().name, cd_order().name])
        .solution_id();
    let settle = test.settle(id).await;

    // Even though the solver returned two solutions, the executed settlement is a
    // combination of the two, meaning the settlements were merged successfully.
    settle
        .ok()
        .await
        .ab_order_executed()
        .await
        .cd_order_executed()
        .await;
}

/// Test that settlements are not merged if the clearing prices don't permit it.
#[tokio::test]
#[ignore]
async fn impossible() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .order(ab_order().rename("second order").reduce_amount(1000000000000000u128.into()))
        // These two solutions result in different clearing prices (due to different surplus),
        // so they can't be merged.
        .solution(ab_solution())
        .solution(Solution {
            orders: vec!["second order"],
            ..ab_solution().reduce_score()
        })
        .done()
        .await;

    // Only the first A-B order gets settled.

    let id = test
        .solve()
        .await
        .ok()
        .orders(&[ab_order().name])
        .solution_id();
    let settle = test.settle(id).await;

    settle.ok().await.ab_order_executed().await;
}
