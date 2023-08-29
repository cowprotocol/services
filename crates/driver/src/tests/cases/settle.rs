use crate::{
    domain::competition::order,
    tests::{
        self,
        cases::{DEFAULT_SOLVER_FEE, DEFAULT_SURPLUS_FEE},
        setup::{ab_order, ab_pool, ab_solution},
    },
};

/// Run a matrix of tests for all meaningful combinations of order kind and
/// side, verifying that they get scored and settled successfully.
#[tokio::test]
#[ignore]
async fn matrix() {
    for side in [order::Side::Buy, order::Side::Sell] {
        for kind in [
            order::Kind::Market,
            order::Kind::Limit {
                surplus_fee: order::SellAmount(DEFAULT_SURPLUS_FEE.into()),
            },
        ] {
            let solver_fee = match kind {
                order::Kind::Market => None,
                order::Kind::Limit { .. } => Some(DEFAULT_SOLVER_FEE.into()),
                order::Kind::Liquidity => None,
            };
            let test = tests::setup()
                .name(format!("{side:?} {kind:?}"))
                .pool(ab_pool())
                .order(ab_order().side(side).kind(kind).solver_fee(solver_fee))
                .solution(ab_solution())
                .done()
                .await;

            test.solve().await.ok().default_score();
            test.settle().await.ok().await.ab_order_executed().await;
        }
    }
}
