use std::time::Duration;
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
    std::{collections::HashSet, sync::Arc},
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
            test.settle(&id).await.ok().await.ab_order_executed().await;
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
async fn discards_excess_settle_requests() {
    let test = Arc::new(
        tests::setup()
            .allow_multiple_solve_requests()
            .pool(ab_pool())
            .order(ab_order())
            .solution(ab_solution())
            .rpc_args(vec!["--no-mining".to_string()])
            .solve_deadline_timeout(chrono::Duration::seconds(4))
            .done()
            .await,
    );

    test.mint_block().await.unwrap();

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

    // `collect_vec` is required to receive results in the same order.
    let settlements = {
        let test_clone = Arc::clone(&test);
        solution_ids
            .iter()
            .cloned()
            .map(move |id| {
                let test_clone = Arc::clone(&test_clone);
                async move { test_clone.settle(&id).await }
            })
            .collect_vec()
    };
    let results_fut = tokio::spawn(join_all(settlements));

    tokio::time::sleep(Duration::from_secs(2)).await;
    test.mint_block().await.unwrap();

    for (index, result) in results_fut.await.unwrap().into_iter().enumerate() {
        match index {
            // The first must be settled.
            0 => {
                result.ok().await.ab_order_executed().await;
            }
            // We don't really care about the intermediate settlements. They are processed but due
            // to the test framework limitation, the same solution settlements fail. We
            // are fine with that to avoid huge changes in the framework.
            1 | 2 => result.err().kind("FailedToSubmit"),
            // Driver's settlement queue max size is 3. Next requests should be discarded.
            3 => result.err().kind("DeadlineExceeded"),
            4 => result.err().kind("DeadlineExceeded"),
            _ => unreachable!(),
        }
    }
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
            .rpc_args(vec!["--no-mining".to_string()])
            .solve_deadline_timeout(chrono::Duration::seconds(4))
            .done()
            .await,
    );

    test.mint_block().await.unwrap();

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

    // Send only first 4 settle requests. `collect_vec` is required to receive
    // results in the same order.
    let first_solutions = {
        let test_clone = Arc::clone(&test);
        solution_ids[..4]
            .iter()
            .cloned()
            .map(move |id| {
                let test_clone = Arc::clone(&test_clone);
                async move { test_clone.settle(&id).await }
            })
            .collect_vec()
    };
    let results_fut = tokio::spawn(join_all(first_solutions));

    tokio::time::sleep(Duration::from_secs(2)).await;
    test.mint_block().await.unwrap();

    for (index, result) in results_fut.await.unwrap().into_iter().enumerate() {
        match index {
            // The first must be settled.
            0 => {
                result.ok().await.ab_order_executed().await;
            }
            // We don't really care about the intermediate settlements. They are processed but due
            // to the test framework limitation, the same solution settlements fail. We
            // are fine with that to avoid huge changes in the framework.
            1 | 2 => result.err().kind("FailedToSubmit"),
            // Driver's settlement queue max size is 3. Next requests should be discarded.
            3 => result.err().kind("DeadlineExceeded"),
            _ => unreachable!(),
        }
    }

    // Wait for the timeout to expire, so all the settle requests are processed.
    // Must a bit higher than `ethrpc_batch_delay`.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    // Now we send the last settlement request.
    test.settle(&solution_ids[4])
        .await
        .err()
        .kind("FailedToSubmit");
}
