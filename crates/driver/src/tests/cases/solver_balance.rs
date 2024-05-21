use crate::{
    domain::eth,
    tests::{
        setup,
        setup::{ab_order, ab_pool, ab_solution, test_solver},
    },
};

/// Test that the `/solve` request errors when solver balance is too low.
#[tokio::test]
#[ignore]
async fn test_unfunded() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .solvers(vec![test_solver()
            .set_name("unfunded")
            .balance(eth::U256::zero())])
        .done()
        .await;

    let solve = test.solve_with_solver("unfunded").await;
    solve.ok().empty();
}

/// Test that the `/solve` request succeeds when the solver has just enough
/// funds
#[tokio::test]
#[ignore]
async fn test_just_enough_funded() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .solvers(vec![test_solver()
            .set_name("barely_funded")
            // The solution uses ~500k gas units
            // With gas costs <20gwei, 0.01 ETH should suffice
            .balance(eth::U256::exp10(16))])
        .done()
        .await;

    test.solve_with_solver("barely_funded").await.ok();
    test.settle_with_solver("barely_funded").await.ok().await;
}
