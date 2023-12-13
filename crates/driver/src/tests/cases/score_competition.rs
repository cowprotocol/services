//! Test that driver properly does competition.

use crate::tests::{
    cases::DEFAULT_SCORE_MIN,
    setup::{ab_order, ab_pool, ab_solution, setup, Score},
};

#[tokio::test]
#[ignore]
async fn solver_score_winner() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().score(Score::Solver { score: 2902421280589416499u128.into()})) // higher than objective value
        .solution(ab_solution().score(Score::RiskAdjusted(0.4)))
        .done()
        .await;

    let solve = test.solve().await.ok();
    assert_eq!(solve.score(), 2902421280589416499u128.into());
    solve.orders(&[ab_order().name]);
    test.reveal().await.ok().calldata();
}

#[tokio::test]
#[ignore]
async fn risk_adjusted_score_winner() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().score(Score::Solver {
            score: DEFAULT_SCORE_MIN.into(),
        }))
        .solution(ab_solution().score(Score::RiskAdjusted(0.9)))
        .done()
        .await;

    let solve = test.solve().await.ok();
    assert!(solve.score() != DEFAULT_SCORE_MIN.into());
    solve.orders(&[ab_order().name]);
    test.reveal().await.ok().calldata();
}
