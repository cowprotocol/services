use crate::tests::{
    self,
    setup::{ab_order, ab_pool, ab_solution},
};

/// A solver can decide to drop a settlement before the autopilot has even
/// asked for it. The withdrawal has to survive that gap, otherwise it is
/// silently ignored and the settlement goes out anyway.
#[tokio::test]
#[ignore]
async fn a_withdrawal_before_settle_stops_the_settlement() {
    let test = tests::setup()
        .name("withdrawal before settle")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .done()
        .await;

    let id = test.solve().await.ok().id();
    assert_eq!(test.cancel(id).await, "recorded");

    test.settle(id).await.err().kind("FailedToSubmit");
}

/// Withdrawals are keyed by solution, so one must not take down a settlement
/// the solver still wants executed.
#[tokio::test]
#[ignore]
async fn a_withdrawal_of_another_solution_leaves_the_settlement_alone() {
    let test = tests::setup()
        .name("withdrawal of another solution")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .done()
        .await;

    let id = test.solve().await.ok().id();
    assert_eq!(test.cancel(id + 1).await, "recorded");

    test.settle(id)
        .await
        .ok()
        .await
        .ab_order_executed(&test)
        .await;
}
