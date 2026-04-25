use crate::tests::setup::{eth_order, eth_solution, setup, test_solver, weth_pool};
#[tokio::test]
async fn solve_returns_no_solutions_when_request_arrives_after_deadline() {
    let test = setup()
        .set_deadline(crate::infra::time::now() - chrono::Duration::milliseconds(50))
        .pool(weth_pool())
        .order(eth_order())
        .solution(eth_solution())
        .done()
        .await;

    test.solve().await.ok().empty();
}

#[tokio::test]
async fn solve_returns_solution_when_within_deadline() {
    let test = setup()
        .set_deadline(crate::infra::time::now() + chrono::Duration::seconds(5))
        .pool(weth_pool())
        .order(eth_order())
        .solution(eth_solution())
        .done()
        .await;

    test.solve().await.ok().solution();
}

#[tokio::test]
async fn solve_returns_no_solutions_when_solver_outlives_deadline() {
    let test = setup()
        .set_deadline(crate::infra::time::now() + chrono::Duration::milliseconds(500))
        .pool(weth_pool())
        .order(eth_order())
        .solution(eth_solution())
        .solvers(vec![
            test_solver().solve_delay(std::time::Duration::from_secs(1)),
        ])
        .done()
        .await;

    test.solve().await.ok().empty();
}

#[tokio::test]
async fn solve_returns_no_solutions_when_all_solvers_outlive_deadline() {
    let test = setup()
        .set_deadline(crate::infra::time::now() + chrono::Duration::milliseconds(500))
        .pool(weth_pool())
        .order(eth_order())
        .solution(eth_solution())
        .solvers(vec![
            test_solver()
                .name("solver1")
                .solve_delay(std::time::Duration::from_secs(1)),
            test_solver()
                .name("solver2")
                .solve_delay(std::time::Duration::from_secs(2)),
        ])
        .done()
        .await;

    let (solver1, solver2) = tokio::join!(
        test.solve_with_solver("solver1"),
        test.solve_with_solver("solver2"),
    );

    solver1.ok().empty();
    solver2.ok().empty();
}

#[tokio::test]
async fn solve_returns_fast_solver_solution_when_slow_solver_outlives_deadline() {
    let test = setup()
        .set_deadline(crate::infra::time::now() + chrono::Duration::seconds(3))
        .pool(weth_pool())
        .order(eth_order())
        .solution(eth_solution())
        .solvers(vec![
            test_solver()
                .name("slow")
                .solve_delay(std::time::Duration::from_secs(4)),
            test_solver().name("fast"),
        ])
        .done()
        .await;

    let (fast, slow) = tokio::join!(
        test.solve_with_solver("fast"),
        test.solve_with_solver("slow"),
    );

    fast.ok().solution();
    slow.ok().empty();
}
