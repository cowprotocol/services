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
    for side in [order::Side::Buy, order::Side::Sell] {
        for kind in [order::Kind::Market, order::Kind::Limit] {
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

/// Checks that settlements with revert risk are not submitted via public
/// mempool.
#[tokio::test]
#[ignore]
async fn private_rpc_with_high_risk_solution() {
    let test = tests::setup()
        .name("private rpc")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .mempools(vec![
            tests::setup::Mempool::Public,
            tests::setup::Mempool::Private {
                url: Some("http://non-existant:8545".to_string()),
            },
        ])
        .done()
        .await;

    test.solve().await.ok().default_score();
    // Public cannot be used and private RPC is not available
    let err = test.settle().await.err();
    err.kind("FailedToSubmit");
}
