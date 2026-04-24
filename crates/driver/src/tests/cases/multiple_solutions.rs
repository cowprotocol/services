use {
    crate::tests::{
        setup,
        setup::{ab_order, ab_pool, ab_solution, ad_order, ad_pool, ad_solution, test_solver},
    },
    alloy::primitives::address,
    eth_domain_types as eth,
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

/// Test that when multiple solutions are proposed they are returned sorted
/// best-first (by score) and each can be revealed independently.
/// Uses two different orders (AB and AD with different sell amounts) so the
/// solutions have genuinely different surplus and therefore different scores.
#[tokio::test]
#[ignore]
async fn all_proposed() {
    let test = setup()
        .solvers(vec![
            test_solver()
                .max_solutions_to_propose(5)
                .submission_account(address!("0000000000000000000000000000000000000001")),
        ])
        .pool(ab_pool())
        .pool(ad_pool())
        .order(ab_order())
        .order(ad_order())
        // Add the lower-scoring solution first so the sort has to reorder.
        .solution(ad_solution())
        .solution(ab_solution())
        .done()
        .await;

    let solve = test.solve().await.ok();
    let solutions = solve.solutions();
    assert_eq!(solutions.len(), 2);

    // Solutions must be sorted best-first (strictly descending score).
    let score0 = solutions[0].get("score").unwrap().as_str().unwrap();
    let score1 = solutions[1].get("score").unwrap().as_str().unwrap();
    let score0 = eth::U256::from_str_radix(score0, 10).unwrap();
    let score1 = eth::U256::from_str_radix(score1, 10).unwrap();
    assert!(score0 > score1, "expected strictly descending scores");

    // Both solutions should be revealable.
    let id0 = solutions[0].get("solutionId").unwrap().as_u64().unwrap();
    let id1 = solutions[1].get("solutionId").unwrap().as_u64().unwrap();
    test.reveal(id0).await.ok().calldata();
    test.reveal(id1).await.ok().calldata();
}

/// Test that `max-solutions-to-propose` caps the number of returned solutions.
#[tokio::test]
#[ignore]
async fn capped() {
    let test = setup()
        .solvers(vec![
            test_solver()
                .max_solutions_to_propose(1)
                .submission_account(address!("0000000000000000000000000000000000000001")),
        ])
        .pool(ab_pool())
        .pool(ad_pool())
        .order(ab_order())
        .order(ad_order())
        .solution(ab_solution())
        .solution(ad_solution())
        .done()
        .await;

    let solve = test.solve().await.ok();
    let solutions = solve.solutions();
    assert_eq!(solutions.len(), 1, "should be capped to 1 solution");
}

/// Test that when multiple solutions are proposed, invalid solutions are
/// discarded and only valid ones are returned.
#[tokio::test]
#[ignore]
async fn only_proposes_valid_solutions() {
    let order = ab_order();
    let test = setup()
        .solvers(vec![
            test_solver()
                .max_solutions_to_propose(5)
                .submission_account(address!("0000000000000000000000000000000000000001")),
        ])
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution())
        .solution(ab_solution().invalid())
        .done()
        .await;

    let solve = test.solve().await.ok();
    let solutions = solve.solutions();
    assert_eq!(solutions.len(), 1);

    let id = solutions[0].get("solutionId").unwrap().as_u64().unwrap();
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
