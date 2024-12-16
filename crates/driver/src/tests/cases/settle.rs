use {
    crate::{
        domain::competition::order,
        tests::{
            self,
            cases::{EtherExt, DEFAULT_SOLVER_FEE},
            setup::{ab_order, ab_pool, ab_solution},
        },
    },
    futures::future::join_all,
    itertools::Itertools,
    std::{collections::HashSet, sync::Arc, time::Duration},
    web3::Transport,
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
                order::Kind::Limit { .. } => Some(DEFAULT_SOLVER_FEE.ether().into_wei()),
            };
            let test = tests::setup()
                .name(format!("{side:?} {kind:?}"))
                .pool(ab_pool())
                .order(ab_order().side(side).kind(kind).solver_fee(solver_fee))
                .solution(ab_solution())
                .done()
                .await;

            let id = test.solve().await.ok().id();
            test.settle(&id)
                .await
                .ok()
                .await
                .ab_order_executed(&test)
                .await;
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

    test.settle("99").await.err().kind("SolutionNotAvailable");
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

    let id = test.solve().await.ok().id();
    // Public cannot be used and private RPC is not available
    test.settle(&id).await.err().kind("FailedToSubmit");
}

#[tokio::test]
#[ignore]
async fn too_much_gas() {
    let test = tests::setup()
        .name("too much gas")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().increase_gas(6_000_000))
        .rpc_args(vec!["--gas-limit".into(), "10000000".into()])
        .done()
        .await;
    test.solve().await.ok().empty();
}

#[tokio::test]
#[ignore]
async fn high_gas_limit() {
    let test = tests::setup()
        .name("high gas limit")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().increase_gas(4_000_000))
        .rpc_args(vec!["--gas-limit".into(), "10000000".into()])
        .done()
        .await;

    let id = test.solve().await.ok().orders(&[ab_order()]).id();

    // Assume validators downvoted gas limit, solution still settles
    test.web3()
        .transport()
        .execute("evm_setBlockGasLimit", vec![serde_json::json!(9_000_000)])
        .await
        .unwrap();
    test.settle(&id).await.ok().await;
}

#[tokio::test]
#[ignore]
async fn discards_excess_settle_and_solve_requests() {
    let test = Arc::new(
        tests::setup()
            .allow_multiple_solve_requests()
            .pool(ab_pool())
            .order(ab_order())
            .solution(ab_solution())
            .settle_submission_deadline(6)
            .done()
            .await,
    );

    // MAX_SOLUTION_STORAGE = 5. Since this is hardcoded, no more solutions can be
    // stored.
    let solution_ids = join_all(vec![
        test.solve(),
        test.solve(),
        test.solve(),
        test.solve(),
        test.solve(),
    ])
    .await
    .into_iter()
    .map(|res| res.ok().id())
    .collect::<Vec<_>>();

    let unique_solutions_count = solution_ids
        .clone()
        .into_iter()
        .collect::<HashSet<_>>()
        .len();
    assert_eq!(unique_solutions_count, solution_ids.len());

    // Disable auto mining to accumulate all the settlement requests.
    test.set_auto_mining(false).await;

    // `collect_vec` is required to receive results in the same order.
    let settlements = {
        let test_clone = Arc::clone(&test);
        solution_ids
            .into_iter()
            .map(|id| {
                let test_clone = Arc::clone(&test_clone);
                async move { test_clone.settle(&id).await }
            })
            .collect_vec()
    };
    let results_fut = tokio::spawn(join_all(settlements));

    tokio::time::sleep(Duration::from_secs(2)).await;
    // While there is no room in the settlement queue, `/solve` requests must be
    // rejected.
    test.solve().await.err().kind("TooManyPendingSettlements");

    // Enable auto mining to process all the settlement requests.
    test.set_auto_mining(true).await;

    let results = results_fut.await.unwrap();
    assert_eq!(results.len(), 5);

    // The first settlement must be successful.
    results[0].clone().ok().await.ab_order_executed(&test).await;

    let err_kinds = results[1..]
        .iter()
        .cloned()
        .counts_by(|settle| settle.err().get_kind());
    let queue_is_full_err_count = *err_kinds.get("TooManyPendingSettlements").unwrap();
    let failed_to_submit_err_count = *err_kinds.get("FailedToSubmit").unwrap();
    // There is a high possibility of a race condition where the first settlement
    // request is dequeued at an unpredictable time. Therefore, we can't guarantee
    // the order of the errors received, but their count must be persistent.
    assert!(queue_is_full_err_count == 2 && failed_to_submit_err_count == 2);

    // `/solve` works again.
    test.solve().await.ok();
}

#[tokio::test]
#[ignore]
async fn accepts_new_settle_requests_after_timeout() {
    let test = Arc::new(
        tests::setup()
            .allow_multiple_solve_requests()
            .pool(ab_pool())
            .order(ab_order())
            .solution(ab_solution())
            .settle_submission_deadline(6)
            .done()
            .await,
    );

    // MAX_SOLUTION_STORAGE = 5. Since this is hardcoded, no more solutions can be
    // stored.
    let solution_ids = join_all(vec![
        test.solve(),
        test.solve(),
        test.solve(),
        test.solve(),
        test.solve(),
    ])
    .await
    .into_iter()
    .map(|res| res.ok().id())
    .collect::<Vec<_>>();

    let unique_solutions_count = solution_ids
        .clone()
        .into_iter()
        .collect::<HashSet<_>>()
        .len();
    assert_eq!(unique_solutions_count, solution_ids.len());

    // Disable auto mining to accumulate all the settlement requests.
    test.set_auto_mining(false).await;

    // Send only first 4 settle requests. `collect_vec` is required to receive
    // results in the same order.
    let first_solutions = {
        let test_clone = Arc::clone(&test);
        solution_ids[..4]
            .iter()
            .cloned()
            .map(|id| {
                let test_clone = Arc::clone(&test_clone);
                async move { test_clone.settle(&id).await }
            })
            .collect_vec()
    };
    let results_fut = tokio::spawn(join_all(first_solutions));

    tokio::time::sleep(Duration::from_secs(1)).await;
    test.set_auto_mining(true).await;

    let results = results_fut.await.unwrap();
    assert_eq!(results.len(), 4);

    // The first settlement must be successful.
    results[0].clone().ok().await.ab_order_executed(&test).await;

    let err_kinds = results[1..]
        .iter()
        .cloned()
        .counts_by(|settle| settle.err().get_kind());
    let queue_is_full_err_count = *err_kinds.get("TooManyPendingSettlements").unwrap();
    let failed_to_submit_err_count = *err_kinds.get("FailedToSubmit").unwrap();
    // There is a high possibility of a race condition where the first settlement
    // request is dequeued at an unpredictable time. The expected count of these
    // errors as follows.
    assert!(
        (queue_is_full_err_count == 1 && failed_to_submit_err_count == 2)
            || (queue_is_full_err_count == 2 && failed_to_submit_err_count == 1)
    );

    // Now we send the last settlement request. It fails because with the current
    // framework setup it is impossible to settle the same solution twice.
    test.settle(&solution_ids[4])
        .await
        .err()
        .kind("FailedToSubmit");
}
