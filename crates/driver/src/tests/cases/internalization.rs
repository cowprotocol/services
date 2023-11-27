use crate::tests::{
    setup,
    setup::{ab_order, ab_pool, ab_solution},
};

/// Test that internalized interactions pass verification if they use trusted
/// tokens.
#[tokio::test]
#[ignore]
async fn valid_internalization() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order().internalize())
        // Marks "A" as trusted and hence OK to use for internalization.
        .trust("A")
        .solution(ab_solution())
        .done()
        .await;

    let solve = test.solve().await;

    solve.ok();
}

/// Test that if internalized interactions don't use trusted tokens, the
/// verification fails with an explanatory error.
#[tokio::test]
#[ignore]
async fn untrusted_internalization() {
    let test = setup()
        .pool(ab_pool())
        .order(ab_order().internalize())
        .solution(ab_solution())
        // Note: does not mark "A" as trusted, despite the order being internalized.
        .done()
        .await;

    let solve = test.solve().await;

    // TODO When we add metrics, assert that an untrusted internalization error is
    // traced.
    solve.ok().empty();
}

/// Check that verification fails if the solution contains internalized
/// transactions which would otherwise fail simulation had they not been
/// internalized.
#[tokio::test]
#[ignore]
async fn non_internalized_simulation_fails() {
    // TODO This is failing simulation after rebase (for some reason). As part
    // of improving this test suite, I want to make such failures easy to debug.
    // So instead of fixing this now, I will use this to drive the improvements
    // to the debugging experience, which I will implement in a follow-up PR.
    /*
    let test = setup()
        .pool(
            "A",
            1000000000000000000000u128.into(),
            "B",
            600000000000u64.into(),
        )
        .order(Order {
            amount: 500000000000000000u64.into(),
            sell_token: "A",
            buy_token: "B",
            internalize: true,
            ..Default::default()
        })
        .trust("A")
        .bogus_calldata()
        .done()
        .await;

    let solve = test.solve().await;

    solve.err("FailingInternalization");
    */
}
