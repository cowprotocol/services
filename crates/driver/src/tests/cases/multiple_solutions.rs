use crate::tests::{
    setup,
    setup::new::{ab_order, ab_solution},
};

/// Test that the best-scoring solution is picked when the /solve endpoint
/// returns multiple valid solutions.
#[tokio::test]
#[ignore]
async fn valid() {
    let test = setup()
        .ab_pool()
        .order(ab_order())
        .solution(ab_solution())
        .solution(ab_solution().reduce_score())
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().orders(&[ab_order().name]).default_score();
}

/// Test that the invalid solution is discarded when the /solve endpoint
/// returns multiple solutions.
#[tokio::test]
#[ignore]
async fn invalid() {
    let test = setup()
        .ab_pool()
        .order(ab_order())
        .solution(ab_solution())
        .solution(ab_solution().reduce_score())
        .solution(ab_solution().invalid())
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().orders(&[ab_order().name]).default_score();
}
