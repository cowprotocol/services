use {
    crate::{
        domain::{competition::order, eth},
        infra::config::file::FeeHandler,
        tests::{
            self,
            setup::{ab_order, ab_pool, ab_solution, test_solver, TRADER_ADDRESS},
        },
    },
    std::str::FromStr,
};

#[tokio::test]
#[ignore]
async fn solver_fee() {
    for side in [order::Side::Buy, order::Side::Sell] {
        let order = ab_order()
            .owner(eth::H160::from_str(TRADER_ADDRESS).unwrap())
            .kind(order::Kind::Limit)
            .side(side)
            .solver_fee(Some(500.into()));
        let test = tests::setup()
            .name(format!("Solver Fee: {side:?}"))
            .solvers(vec![test_solver().fee_handler(FeeHandler::Driver)])
            .pool(ab_pool())
            .order(order.clone())
            .solution(ab_solution())
            .done()
            .await;

        test.solve().await.ok().orders(&[order]);
    }
}
