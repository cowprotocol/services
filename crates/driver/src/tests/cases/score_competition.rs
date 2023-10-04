//! Test that driver properly does competition.

use crate::tests::{
    cases::{DEFAULT_SCORE_MAX, DEFAULT_SCORE_MIN},
    setup::{ab_order, ab_pool, ab_solution, setup, Score},
};

#[tokio::test]
#[ignore]
async fn solver_score_winner() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().score(Score::Solver(DEFAULT_SCORE_MAX.into())))
        .solution(ab_solution().score(Score::RiskAdjusted(0.6)))
        .done()
        .await;

    assert_eq!(test.solve().await.ok().score(), DEFAULT_SCORE_MAX.into());
    test.reveal().await.ok().orders(&[ab_order().name]);
}

#[tokio::test]
#[ignore]
async fn risk_adjusted_score_winner() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().score(Score::Solver(DEFAULT_SCORE_MIN.into())))
        .solution(ab_solution().score(Score::RiskAdjusted(0.9)))
        .done()
        .await;

    assert!(test.solve().await.ok().score() != DEFAULT_SCORE_MIN.into());
    test.reveal().await.ok().orders(&[ab_order().name]);
}
