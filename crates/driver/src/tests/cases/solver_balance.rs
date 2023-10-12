use crate::tests::{
    setup,
    setup::{ab_order, ab_pool, ab_solution},
};

/// Test that the `/solve` request errors when solver balance is too low.
#[tokio::test]
#[ignore]
async fn test() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        // The solver gets defunded.
        .defund_solver()
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().empty();
}
