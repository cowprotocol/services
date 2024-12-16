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
    std::{sync::Arc, time::Duration},
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

    let unique_solutions_count = solution_ids.iter().unique().count();
    assert_eq!(unique_solutions_count, solution_ids.len());

    // Disable auto mining to accumulate all the settlement requests.
    test.set_auto_mining(false).await;

    // To avoid race conditions with the settlement queue processing, a
    // `/settle` request needs to be sent first, so it is dequeued, and it's
    // execution is paused before any subsequent request is received.
    let test_clone = Arc::clone(&test);
    let first_solution_id = solution_ids[0].clone();
    let first_settlement_fut =
        tokio::spawn(async move { test_clone.settle(&first_solution_id).await });
    // Make sure the first settlement gets dequeued before sending the remaining
    // requests.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let remaining_solutions = solution_ids[1..].to_vec();
    let remaining_settlements = {
        let test_clone = Arc::clone(&test);
        remaining_solutions.into_iter().map(move |id| {
            let test_clone = Arc::clone(&test_clone);
            async move { test_clone.settle(&id).await }
        })
    };
    let remaining_settlements_fut = tokio::spawn(join_all(remaining_settlements));

    // Sleep for a bit to make sure all the settlement requests are queued.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // While there is no room in the settlement queue, `/solve` requests must be
    // rejected.
    test.solve().await.err().kind("TooManyPendingSettlements");

    // Enable auto mining to process all the settlement requests.
    test.set_auto_mining(true).await;

    // The first settlement must be successful.
    let first_settlement = first_settlement_fut.await.unwrap();
    first_settlement.ok().await.ab_order_executed(&test).await;

    let remaining_settlements = remaining_settlements_fut.await.unwrap();
    assert_eq!(remaining_settlements.len(), 4);

    for (idx, result) in remaining_settlements.into_iter().enumerate() {
        match idx {
            // The next 2 settlements failed to submit due to the framework's limitation(unable to
            // fulfill the same order again).
            0 | 1 => result.err().kind("FailedToSubmit"),
            // All the subsequent settlements rejected due to the settlement queue being full.
            2 | 3 => result.err().kind("TooManyPendingSettlements"),
            _ => unreachable!(),
        }
    }

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

    let unique_solutions_count = solution_ids.iter().unique().count();
    assert_eq!(unique_solutions_count, solution_ids.len());

    // Disable auto mining to accumulate all the settlement requests.
    test.set_auto_mining(false).await;

    // To avoid race conditions with the settlement queue processing, a
    // `/settle` request needs to be sent first, so it is dequeued, and it's
    // execution is paused before any subsequent request is received.
    let test_clone = Arc::clone(&test);
    let first_solution_id = solution_ids[0].clone();
    let first_settlement_fut =
        tokio::spawn(async move { test_clone.settle(&first_solution_id).await });
    // Make sure the first settlement gets dequeued before sending the remaining
    // requests.
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Send only 3 more settle requests.
    let additional_solutions = solution_ids[1..4].to_vec();
    let additional_settlements = {
        let test_clone = Arc::clone(&test);
        additional_solutions.into_iter().map(move |id| {
            let test_clone = Arc::clone(&test_clone);
            async move { test_clone.settle(&id).await }
        })
    };
    let additional_settlements_fut = tokio::spawn(join_all(additional_settlements));

    // Sleep for a bit to make sure all the settlement requests are queued.
    tokio::time::sleep(Duration::from_millis(500)).await;
    test.set_auto_mining(true).await;

    let first_settlement = first_settlement_fut.await.unwrap();
    // The first settlement must be successful.
    first_settlement.ok().await.ab_order_executed(&test).await;

    let additional_settlements = additional_settlements_fut.await.unwrap();
    assert_eq!(additional_settlements.len(), 3);

    for (idx, result) in additional_settlements.into_iter().enumerate() {
        match idx {
            // The next 2 settlements failed to submit due to the framework's limitation(unable to
            // fulfill the same order again).
            0 | 1 => result.err().kind("FailedToSubmit"),
            // The next request gets rejected due to the settlement queue being full.
            2 => result.err().kind("TooManyPendingSettlements"),
            _ => unreachable!(),
        }
    }

    // Now we send the last settlement request. It fails due to the framework's
    // limitation(unable to fulfill the same order again).
    test.settle(&solution_ids[4])
        .await
        .err()
        .kind("FailedToSubmit");
}
