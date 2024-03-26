use crate::{
    domain::competition::order,
    infra::config::file::FeeHandler,
    tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution, test_solver},
    },
};

#[tokio::test]
#[ignore]
async fn rejects_unwarranted_solver_fee() {
    let test = tests::setup()
        .name("Solver fee on market order".to_string())
        .pool(ab_pool())
        .order(
            // A solver reporting a fee on a swap order
            ab_order()
                .user_fee(1000.into())
                .solver_fee(Some(500.into())),
        )
        .solution(ab_solution())
        .done()
        .await;

    test.solve().await.status(hyper::StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore]
async fn solver_fee() {
    for side in [order::Side::Buy, order::Side::Sell] {
        let order = ab_order()
            .kind(order::Kind::Limit)
            .side(side)
            .solver_fee(Some(500.into()));
        let test = tests::setup()
            .name(format!("Solver Fee: {side:?}"))
            .solvers(vec![test_solver().fee_handler(FeeHandler::Solver)])
            .pool(ab_pool())
            .order(order.clone())
            .solution(ab_solution())
            .done()
            .await;

        test.solve().await.ok().orders(&[order]);
    }
}

#[tokio::test]
#[ignore]
async fn user_fee() {
    for side in [order::Side::Buy, order::Side::Sell] {
        let order = ab_order().side(side).user_fee(1000.into());
        let test = tests::setup()
            .name(format!("User Fee: {side:?}"))
            .pool(ab_pool())
            .order(order.clone())
            .solution(ab_solution())
            .done()
            .await;

        test.solve().await.ok().orders(&[order]);
    }
}
