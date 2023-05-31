use crate::tests::{
    setup,
    setup::new::{ab_order, ab_solution},
};

/// Test that the /settle endpoint broadcasts a valid settlement transaction.
#[tokio::test]
#[ignore]
async fn test() {
    let test = setup()
        .ab_pool()
        .order(ab_order())
        .solution(ab_solution())
        .done()
        .await;

    let id = test.solve().await.ok().solution_id();
    let settle = test.settle(id).await;

    settle.ok().await.ab_order_executed().await;
}
