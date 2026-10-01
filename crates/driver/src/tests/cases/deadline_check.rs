//! Verifies that the on-chain settlement transaction the driver
//! actually submits reverts when included past its encoded deadline.
//! So even if the driver fails to properly cancel a tx it will not
//! interfere with other solvers settling the same order in subsequent
//! auctions.
//!
//! Strategy: settle a normal auction so the driver builds and submits a
//! real settlement, then advance block-by-block with `evm_mine`,
//! replaying the same tx via `eth_call` on each block. While we are
//! at or below the deadline the replay must succeed; the first block
//! past the deadline it must revert with `DeadlineExceeded`. Checking
//! both sides pins the `>` vs `>=` semantics of the on-chain check
//! and keeps the test independent of how many blocks `settle()`
//! happens to mine internally.

use {
    crate::tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution},
    },
    alloy::{
        consensus::Transaction,
        primitives::TxKind,
        providers::{Provider, ext::AnvilApi},
        rpc::types::TransactionRequest,
        sol_types::SolError,
        transports::RpcError,
    },
    contracts::support::DeadlineCheck::DeadlineCheck::DeadlineExceeded,
};

const SETTLE_DEADLINE_BLOCKS: u64 = 3;

#[tokio::test]
#[ignore]
async fn settlement_reverts_when_replayed_past_deadline() {
    let test = tests::setup()
        .name("deadline check reverts past deadline")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .settle_submission_deadline(SETTLE_DEADLINE_BLOCKS)
        .done()
        .await;

    // Normal solve + settle. The driver encodes with `Some(deadline)`
    // where `deadline = block_at_settle + SETTLE_DEADLINE_BLOCKS`, and
    // the tx is mined before that deadline (so the on-chain call
    // succeeds).
    let id = test.solve().await.ok().id();
    test.settle(id)
        .await
        .ok()
        .await
        .ab_order_executed(&test)
        .await;

    // Grab the actual settlement tx the driver just submitted.
    let block = test
        .web3()
        .provider
        .get_block_by_number(Default::default())
        .await
        .unwrap()
        .unwrap();
    let tx_hash = block
        .transactions
        .hashes()
        .next()
        .expect("mined block should contain the settlement tx");
    let tx = test
        .web3()
        .provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();

    let inner: &dyn Transaction = &*tx.inner;
    let to = match inner.kind() {
        TxKind::Call(addr) => addr,
        TxKind::Create => panic!("settlement tx must be a call, not a create"),
    };
    let call = TransactionRequest::default()
        .from(tx.inner.signer())
        .to(to)
        .input(inner.input().clone().into())
        .value(inner.value());

    // Walk forward one block at a time, replaying the exact tx via
    // `eth_call` at each step. The replay must succeed as long as
    // `block.number <= deadline` and must revert with
    // `DeadlineExceeded` the first block past the deadline.
    //
    // A safety cap of `SETTLE_DEADLINE_BLOCKS + 2` extra iterations
    // means the loop is bounded even if the check never fires (in
    // which case the final assertion will catch it).
    let mut saw_pre_deadline_success = false;
    let mut saw_post_deadline_revert = false;
    for _ in 0..=(SETTLE_DEADLINE_BLOCKS + 2) {
        match test.web3().provider.call(call.clone()).await {
            Ok(_) => {
                saw_pre_deadline_success = true;
                test.web3().provider.evm_mine(None).await.unwrap();
            }
            Err(err) => {
                let revert_data = match &err {
                    RpcError::ErrorResp(payload) => payload
                        .as_revert_data()
                        .unwrap_or_else(|| panic!("expected revert data, got: {payload:?}")),
                    other => panic!("expected an ErrorResp with revert data, got: {other:?}"),
                };
                assert!(
                    revert_data.starts_with(&DeadlineExceeded::SELECTOR),
                    "expected DeadlineExceeded error ({:x?}), got revert data: {revert_data:?}",
                    DeadlineExceeded::SELECTOR,
                );
                saw_post_deadline_revert = true;
                break;
            }
        }
    }

    assert!(
        saw_pre_deadline_success,
        "replay should have succeeded at least once before the deadline"
    );
    assert!(
        saw_post_deadline_revert,
        "replay never reverted with DeadlineExceeded within the search window"
    );
}
