use {
    crate::{
        domain::competition::order,
        tests::{
            self,
            cases::{DEFAULT_SOLVER_FEE, EtherExt},
            setup::{ab_order, ab_pool, ab_solution},
        },
    },
    alloy::providers::Provider,
    futures::future::join_all,
    itertools::Itertools,
    std::{sync::Arc, time::Duration},
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
                order::Kind::Limit => Some(DEFAULT_SOLVER_FEE.ether().into_wei()),
            };
            let test = tests::setup()
                .name(format!("{side:?} {kind:?}"))
                .pool(ab_pool())
                .order(ab_order().side(side).kind(kind).solver_fee(solver_fee))
                .solution(ab_solution())
                .done()
                .await;

            let id = test.solve().await.ok().id();
            test.settle(id)
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

    test.settle(99).await.err().kind("SolutionNotAvailable");
}

/// Checks that settlements with revert risk are not submitted via public
/// mempool if at least 1 revert protected mempool exists.
#[tokio::test]
#[ignore]
async fn private_rpc_with_high_risk_solution() {
    let test = tests::setup()
        .name("private rpc")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .mempools(vec![
            tests::setup::Mempool::Default,
            tests::setup::Mempool::Private {
                url: Some("http://non-existant:8545".to_string()),
                mines_reverting_txs: false,
            },
        ])
        .done()
        .await;

    let id = test.solve().await.ok().id();
    // Public cannot be used and private RPC is not available
    test.settle(id).await.err().kind("FailedToSubmit");
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
async fn submits_huge_solution() {
    let test = tests::setup()
        .name("high gas limit")
        .allow_multiple_solve_requests()
        .pool(ab_pool())
        .order(ab_order())
        // the solution will end up using 5049130 gas, let's set 11M block limit so it's <half
        .solution(ab_solution().increase_gas(2_000_000))
        .rpc_args(vec!["--gas-limit".into(), "11000000".into()])
        .done()
        .await;

    let id = test.solve().await.ok().orders(&[ab_order()]).id();

    // Assume validators downvoted gas limit, solution still settles: even though we
    // have a rule that in the _bidding phase_ the solution needs to use less than
    // half of the block gas limit, we want it to be submitted/settled as long as it
    // fits in the block.
    test.web3()
        .provider
        .raw_request::<_, bool>("evm_setBlockGasLimit".into(), (9_000_000,))
        .await
        .unwrap();
    test.settle(id).await.ok().await;
}

#[tokio::test]
#[ignore]
async fn does_not_bid_huge_solution() {
    let test = tests::setup()
        .name("high gas limit")
        .pool(ab_pool())
        .order(ab_order())
        // the solution will end up using 5049130 gas, which is >half of block gas limit
        .solution(ab_solution().increase_gas(2_000_000))
        .rpc_args(vec!["--gas-limit".into(), "10000000".into()])
        .done()
        .await;

    // The found solution is bigger than half of gas block limit which means it gets
    // discarded
    test.solve().await.ok().empty();
}

/// Verifies the admission semaphore correctly limits in-flight settle requests
/// to `pool_slots + settle_queue_size` (default 1 + 2 = 3). We can submit as
/// many bids as we want as long as there is at least 1 settle queue spot
/// available, but once the queue is full, new /solve requests are rejected.
/// After the settlements complete, capacity is restored and new bids can be
/// submitted.
#[tokio::test]
#[ignore]
async fn settle_queue_capacity_is_respected() {
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

    // Disable auto mining so settlements block on confirmation.
    test.set_auto_mining(false).await;

    // Send settle requests one at a time so admission is deterministic.
    // Admission capacity = 1 (pool slot) + 2 (settle_queue_size) = 3.
    let mut settle_futs = Vec::new();
    for &id in &solution_ids {
        let test_clone = Arc::clone(&test);
        settle_futs.push(tokio::spawn(async move { test_clone.settle(id).await }));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Wait for all requests to be either in-flight or rejected.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // While all admission slots are taken, /solve requests must be rejected.
    test.solve().await.err().kind("TooManyPendingSettlements");

    // Enable auto mining to process all the settlement requests.
    // *Note that processing the settlement requests will change the gas
    // estimates!*
    test.set_auto_mining(true).await;

    let results: Vec<_> = join_all(settle_futs)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    let mut results = results.into_iter();

    // The first settle should succeed on-chain.
    results
        .next()
        .unwrap()
        .ok()
        .await
        .ab_order_executed(&test)
        .await;

    for (idx, result) in results.enumerate() {
        match idx {
            // Admitted but can't fulfill the same order again.
            0 | 1 => result.err().kind("FailedToSubmit"),
            // Rejected by the admission semaphore.
            2 | 3 => result.err().kind("TooManyPendingSettlements"),
            _ => unreachable!(),
        }
    }

    // Capacity is restored — /solve works again.
    test.solve().await.ok();
}
