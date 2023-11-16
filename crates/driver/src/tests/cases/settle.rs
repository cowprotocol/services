use crate::{
    domain::competition::order,
    tests::{
        self,
        cases::DEFAULT_SOLVER_FEE,
        setup::{ab_order, ab_pool, ab_solution},
    },
};

/// Run a matrix of tests for all meaningful combinations of order kind and
/// side, verifying that they get scored and settled successfully.
#[tokio::test]
#[ignore]
async fn matrix() {
    let rt = tokio::runtime::Handle::current();

    for side in [order::Side::Buy, order::Side::Sell] {
        for kind in [order::Kind::Market, order::Kind::Limit] {
            let solver_fee = match kind {
                order::Kind::Market => None,
                order::Kind::Limit { .. } => Some(DEFAULT_SOLVER_FEE.into()),
                order::Kind::Liquidity => None,
            };
            // need to execute sequentially to make sure the Test struct is created
            // correctly for each test (specifially the deadline, since we don't want to
            // build deadline for all tests, and then execute tests sequentially, which
            // would make some deadlines expired before even starting the test)
            rt.block_on(async {
                let test = tests::setup()
                    .name(format!("{side:?} {kind:?}"))
                    .pool(ab_pool())
                    .order(ab_order().side(side).kind(kind).solver_fee(solver_fee))
                    .solution(ab_solution())
                    .done()
                    .await;

                test.solve().await.ok().default_score();
                test.settle().await.ok().await.ab_order_executed().await;
            });
        }
    }
}

/// Checks that settling without a solution returns an error.
#[tokio::test]
#[ignore]
async fn solution_not_available() {
    let test = tests::setup()
        .name("solution not available")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .done()
        .await;

    test.settle().await.err().kind("SolutionNotAvailable");
}
