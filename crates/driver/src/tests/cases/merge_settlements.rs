use crate::tests::{
    cases::IntoWei,
    setup::{self, ab_order, ab_pool, ab_solution, cd_order, cd_pool, cd_solution, Solution, Test},
};

/// Test that settlements can be merged.
#[tokio::test]
#[ignore]
async fn possible() {
    let ab_order = ab_order();
    let cd_order = cd_order();
    let test: Test = setup::setup()
        .pool(cd_pool())
        .pool(ab_pool())
        .order(ab_order.clone())
        .order(cd_order.clone())
        .solution(cd_solution())
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.ok().orders(&[ab_order, cd_order]);
    test.reveal().await.ok().calldata();
    test.settle()
        .await
        // Even though the solver returned two solutions, the executed settlement is a
        // combination of the two, meaning the settlements were merged successfully.
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
    let order = ab_order();
    let test = setup::setup()
        .pool(ab_pool())
        .order(order.clone())
        .order(order.clone().rename("reduced order").reduce_amount(1e-3.into_wei()))
        // These two solutions result in different clearing prices (due to different surplus),
        // so they can't be merged.
        .solution(ab_solution())
        .solution(Solution {
            orders: vec!["reduced order"],
            ..ab_solution().reduce_score()
        })
        .done()
        .await;

    // Only the first A-B order gets settled.

    test.solve().await.ok().orders(&[order]);
    test.reveal().await.ok().calldata();
    test.settle().await.ok().await.ab_order_executed().await;
}
