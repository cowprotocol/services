use crate::tests::{
    setup,
    setup::new::{ab_order, ab_solution},
};

/// Test that the /solve endpoint calculates the correct score.
#[tokio::test]
#[ignore]
async fn test() {
    let test = setup()
        .ab_pool()
        .order(ab_order())
        .solution(ab_solution())
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().orders(&[ab_order().name]).default_score();
}
