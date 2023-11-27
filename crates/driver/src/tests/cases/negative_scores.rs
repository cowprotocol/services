//! Test that solutions with negative scores get skipped.

use crate::tests::{
    setup,
    setup::{ab_order, ab_pool, ab_solution, Solution},
};

#[tokio::test]
#[ignore]
async fn no_valid_solutions() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order().no_surplus())
        // The solution has no surplus, and hence a negative score.
        .solution(ab_solution())
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().empty();
}

#[tokio::test]
#[ignore]
async fn one_valid_solution() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .order(ab_order().rename("no surplus").no_surplus())
        .solution(ab_solution())
        // This solution has no surplus, and hence a negative score, so it gets skipped.
        .solution(Solution {
            orders: vec!["no surplus"],
            ..ab_solution()
        })
        .done()
        .await;
    test.solve().await.ok().default_score();
    test.reveal().await.ok().orders(&[ab_order().name]);
}
