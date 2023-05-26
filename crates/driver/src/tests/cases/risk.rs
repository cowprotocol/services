use crate::tests::{
    self,
    cases::{DEFAULT_SCORE_MAX, DEFAULT_SCORE_MIN},
    setup::{ab_order, ab_pool, ab_solution},
};

pub const RISK: u128 = 100000000000000000u128;

/// Test that the solution risk affects the score.
#[tokio::test]
#[ignore]
async fn test() {
    let test = tests::setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().risk(RISK.into()))
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok().orders(&[ab_order().name]).score(
        (DEFAULT_SCORE_MIN - RISK).into(),
        (DEFAULT_SCORE_MAX - RISK).into(),
    );
}
