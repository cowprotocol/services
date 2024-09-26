use crate::tests::setup::{eth_order, eth_solution, setup, weth_pool};

/// Tests simple happy case where the driver knows about a single
/// solution and is told to execute it.
#[tokio::test]
#[ignore]
async fn driver_handles_solutions_based_on_id() {
    let order = eth_order();
    let test = setup()
        .pool(weth_pool())
        .order(order.clone())
        .solution(eth_solution())
        .done()
        .await;

    let id = test.solve().await.ok().id();

    // calling `/reveal` or `/settle` with incorrect solution ids
    // results in an error.
    test.settle("123123")
        .await
        .err()
        .kind("SolutionNotAvailable");
    test.reveal("123123")
        .await
        .err()
        .kind("SolutionNotAvailable");

    // calling `/reveal` or `/settle` with a reasonable id works.
    test.reveal(&id).await.ok();
    test.settle(&id).await.ok().await.eth_order_executed().await;

    // calling `/reveal` or `/settle` with for a legit solution that
    // has already been settled also fail.
    test.settle(&id).await.err().kind("SolutionNotAvailable");
    test.reveal(&id).await.err().kind("SolutionNotAvailable");
}

/// Tests that the driver can correctly settle a solution that
/// was not the most recent one.
#[tokio::test]
#[ignore]
async fn driver_can_settle_old_solutions() {
    let order = eth_order();
    let test = setup()
        .allow_multiple_solve_requests()
        .pool(weth_pool())
        .order(order.clone())
        .solution(eth_solution())
        .done()
        .await;

    let id1 = test.solve().await.ok().id();
    let id2 = test.solve().await.ok().id();
    let id3 = test.solve().await.ok().id();

    // all solution ids are unique
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);

    // Driver is able to settle older solutions
    // Technically this is not super convincing since all remembered solutions
    // are identical but this is the best we are going to get without needing
    // to heavily modify the testing framework.
    test.settle(&id1)
        .await
        .ok()
        .await
        .eth_order_executed()
        .await;
}

/// Tests that the driver only remembers a relatively short number
/// of solutions.
#[tokio::test]
#[ignore]
async fn driver_has_a_short_memory() {
    let order = eth_order();
    let test = setup()
        .allow_multiple_solve_requests()
        .pool(weth_pool())
        .order(order.clone())
        .solution(eth_solution())
        .done()
        .await;

    let id1 = test.solve().await.ok().id();
    let id2 = test.solve().await.ok().id();
    let id3 = test.solve().await.ok().id();
    let id4 = test.solve().await.ok().id();
    let id5 = test.solve().await.ok().id();
    let id6 = test.solve().await.ok().id();

    // recalling the 5 most recent solutions works
    test.reveal(&id2).await.ok();
    test.reveal(&id3).await.ok();
    test.reveal(&id4).await.ok();
    test.reveal(&id5).await.ok();
    test.reveal(&id6).await.ok();

    // recalling an older solution doesn't work
    test.reveal(&id1).await.err().kind("SolutionNotAvailable");
}
