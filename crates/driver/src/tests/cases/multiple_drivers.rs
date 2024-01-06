//! Test that driver properly works when running multiple instances.

use crate::tests::setup::{ab_order, ab_pool, ab_solution, setup, test_solver};

#[tokio::test]
#[ignore]
async fn separate_deadline() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .solver(test_solver().name("second").solving_time_share(0.5))
        .done()
        .await;

    test.solve().await.ok();
    test.solve_with_solver("second").await.ok();
}
