use crate::tests::{
    cases::EtherExt,
    setup::{
        self,
        ab_order,
        ab_pool,
        ab_solution,
        cd_order,
        cd_pool,
        cd_solution,
        test_solver,
        Solution,
        Test,
    },
};

/// Test that settlements can be merged.
#[tokio::test]
#[ignore]
async fn possible() {
    let ab_order = ab_order();
    let cd_order = cd_order();
    let test: Test = setup::setup()
        .solvers(vec![test_solver().merge_solutions()])
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
        .solvers(vec![test_solver().merge_solutions()])
        .pool(ab_pool())
        .order(order.clone())
        .order(order.clone().rename("reduced order").reduce_amount("1e-3".ether().into_wei()))
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

/// Test that mergable solutions don't get merged if feature was not enabled.
#[tokio::test]
#[ignore]
async fn possible_but_forbidden() {
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

    // Even though the solutions could be combined (see test "possible") they were
    // not because solution merging is not enabled by default.
    test.solve().await.ok().orders(&[ab_order]);
    test.reveal().await.ok().calldata();
    test.settle().await.ok().await.ab_order_executed().await;
}
