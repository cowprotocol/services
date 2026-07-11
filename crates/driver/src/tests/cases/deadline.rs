use {
    crate::tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution, test_solver},
    },
    alloy::{primitives::b256, signers::local::PrivateKeySigner},
    std::time::Duration,
};

const SOLUTION_COUNT: usize = 30;

fn submission_account() -> PrivateKeySigner {
    // Well-known Anvil test key #1. Do not use as a production key.
    PrivateKeySigner::from_bytes(&b256!(
        "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
    ))
    .unwrap()
}

#[tokio::test]
#[ignore]
async fn expired_solver_deadline_does_not_send_solve_request() {
    let test = tests::setup()
        .name("expired solver deadline")
        .solvers(vec![test_solver().solving_time_share(0.0)])
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.ok().empty();

    assert_eq!(test.solve_requests(), 0);
}

#[tokio::test]
#[ignore]
async fn postprocessing_timeout_drops_late_solution() {
    let mut setup = tests::setup()
        .name("postprocessing timeout")
        .solvers(vec![
            test_solver()
                .solving_time_share(1.0)
                .max_solutions_to_propose(SOLUTION_COUNT)
                .submission_account(submission_account())
                .post_processing_concurrency_limit(1),
        ])
        .deadline_after(Duration::from_millis(600))
        .pool(ab_pool())
        .order(ab_order());

    for _ in 0..SOLUTION_COUNT {
        setup = setup.solution(ab_solution());
    }

    let test = setup.done().await;

    let solve = test.solve().await.ok();
    let solutions = solve.solutions();

    assert!(
        solutions.len() < SOLUTION_COUNT,
        "expected post-processing timeout to cut off late solutions"
    );

    assert_eq!(test.solve_requests(), 1);
}

#[tokio::test]
#[ignore]
async fn settlement_rejects_expired_submission_deadline() {
    let test = tests::setup()
        .name("expired settlement submission deadline")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .settle_submission_deadline(0)
        .done()
        .await;

    let solution_id = test.solve().await.ok().id();

    test.settle(solution_id)
        .await
        .err()
        .kind("DeadlineExceeded");
}
