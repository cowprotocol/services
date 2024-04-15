//! Test that driver properly works when running multiple instances.

use {
    crate::{
        domain::eth,
        tests::setup::{ab_order, ab_pool, ab_solution, setup, test_solver, TRADER_ADDRESS},
    },
    std::str::FromStr,
};

#[tokio::test]
#[ignore]
async fn separate_deadline() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order().owner(eth::H160::from_str(TRADER_ADDRESS).unwrap()))
        .solution(ab_solution())
        .solvers(vec![
            test_solver().name("first"),
            test_solver().name("second").solving_time_share(0.5),
        ])
        .done()
        .await;

    test.solve_with_solver("first").await.ok();
    test.solve_with_solver("second").await.ok();
}
