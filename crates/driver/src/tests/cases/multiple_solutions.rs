use {
    crate::tests::{
        setup,
        setup::{ab_order, ab_pool, ab_solution, test_solver},
    },
    alloy::primitives::address,
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

/// Test that when `propose-all-solutions` is enabled, all valid solutions are
/// returned (sorted best-first) and each can be revealed.
#[tokio::test]
#[ignore]
async fn all_proposed() {
    let order = ab_order();
    let test = setup()
        .solvers(vec![
            test_solver()
                .propose_all_solutions()
                .submission_account(address!("0000000000000000000000000000000000000001")),
        ])
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution())
        .solution(ab_solution().reduce_score())
        .done()
        .await;

    let solve = test.solve().await.ok();
    let solutions = solve.solutions();
    assert_eq!(solutions.len(), 2);

    // Both solutions should be revealable.
    let id0 = solutions[0].get("solutionId").unwrap().as_u64().unwrap();
    let id1 = solutions[1].get("solutionId").unwrap().as_u64().unwrap();
    test.reveal(id0).await.ok().calldata();
    test.reveal(id1).await.ok().calldata();
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
