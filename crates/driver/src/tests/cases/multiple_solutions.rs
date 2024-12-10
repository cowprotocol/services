use crate::tests::{
    setup,
    setup::{ab_order, ab_pool, ab_solution},
};

/// Test that the best-scoring solution is picked when the /solve endpoint
/// returns multiple valid solutions.
#[tokio::test]
#[ignore]
async fn valid() {
    let order = ab_order();
    let test = setup()
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution())
        .solution(ab_solution().reduce_score())
        .done()
        .await;

    let id = test.solve().await.ok().orders(&[order]).id();
    test.reveal(id).await.ok().calldata();
}

/// Test that the invalid solution is discarded when the /solve endpoint
/// returns multiple solutions.
#[tokio::test]
#[ignore]
async fn invalid() {
    let order = ab_order();
    let test = setup()
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution().reduce_score())
        .solution(ab_solution().invalid())
        .done()
        .await;

    let id = test.solve().await.ok().orders(&[order]).id();
    test.reveal(id).await.ok().calldata();
}
