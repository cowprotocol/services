//! Verifies that the on-chain settlement transaction the driver
//! actually submits reverts when included past its encoded deadline.
//! So even if the driver fails to properly cancel a tx it will not
//! interfere with other solvers settling the same order in subsequent
//! auctions.
//!
//! Strategy: settle a normal auction so the driver builds and submits a
//! real settlement, then time-travel with `evm_mine` and replay the
//! same tx via `eth_call`. If the driver injected the `DeadlineCheck`
//! pre-interaction correctly (untrampolined, deadline set to
//! `submission_deadline`), the replay must revert with
//! `DeadlineExceeded` — because the check runs as the very first step
//! of the batch and now sees `block.number > deadline`.

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

    // Advance the blockchain to the first block where the tx should
    // fail.
    for _ in 0..(SETTLE_DEADLINE_BLOCKS - 1) {
        test.web3().provider.evm_mine(None).await.unwrap();
    }

    // Replay the exact tx via eth_call. If the DeadlineCheck
    // pre-interaction is present and correctly configured, it runs
    // first and reverts the whole batch with `DeadlineExceeded`. If the
    // driver had *not* injected the check (or had trampolined it), the
    // call would either succeed or revert somewhere deeper in the
    // settlement pipeline — either way, not with this selector.
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

    let err = test
        .web3()
        .provider
        .call(call)
        .await
        .expect_err("replay past deadline must revert");

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
}
