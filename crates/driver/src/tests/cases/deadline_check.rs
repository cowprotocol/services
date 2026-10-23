use {
    crate::tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution},
    },
    alloy::{
        consensus::Transaction,
        primitives::{TxKind, U256},
        providers::Provider,
        rpc::types::TransactionRequest,
        sol_types::SolError,
        transports::RpcError,
    },
    contracts::support::DeadlineCheck::DeadlineCheck::DeadlineExceeded,
};

/// Verifies that the on-chain settlement transaction the driver
/// actually submits reverts when included past its encoded deadline.
/// So even if the driver fails to properly cancel a tx it will not
/// interfere with other solvers settling the same order in subsequent
/// auctions.
///
/// Strategy: ask the driver to settle with the tightest possible
/// deadline (1 block), which the driver meets by mining the settlement
/// in the very next block. Due to anvil's automine the settlement will
/// advance the block chain by 1 block. So by the time `settle()`
/// returns we are already at `deadline + 1`. Replaying the exact same
/// tx via `eth_call` at that point must therefore fire the `DeadlineCheck`
/// pre-interaction and revert with `DeadlineExceeded`.
#[tokio::test]
#[ignore]
async fn settlement_reverts_when_replayed_past_deadline() {
    let test = tests::setup()
        .name("deadline check reverts past deadline")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        // Tightest possible deadline: the tx is only valid for the
        // block immediately after submission. Any later block must
        // revert with `DeadlineExceeded`.
        .settle_submission_deadline(1)
        .done()
        .await;

    // Normal solve + settle. The driver encodes with `Some(deadline)`
    // where `deadline = block_at_settle + 1`, and the tx is mined in
    // that very block (so the on-chain check passes).
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

    // The settlement advanced the chain by 1 block so now replaying
    // the tx has to revert with the `DeadlineExceeded` error.
    let err = test
        .web3()
        .provider
        .call(call)
        .await
        .expect_err("replay one block past the deadline must revert");
    let revert_data = match &err {
        RpcError::ErrorResp(payload) => payload
            .as_revert_data()
            .unwrap_or_else(|| panic!("expected revert data, got: {payload:?}")),
        other => panic!("expected an ErrorResp with revert data, got: {other:?}"),
    };
    assert!(
        revert_data.starts_with(&DeadlineExceeded::SELECTOR),
        "expected DeadlineExceeded ({:x?}), got revert data: {revert_data:?}",
        DeadlineExceeded::SELECTOR,
    );

    // Pin the exact `>` semantics: we only advanced a single block
    // past the deadline, so the contract must report the current
    // block as `deadline + 1`. A `>=` on-chain check would have
    // reverted the real settle itself and we'd never have reached
    // this point.
    let decoded =
        DeadlineExceeded::abi_decode(&revert_data).expect("DeadlineExceeded payload must decode");
    assert_eq!(
        decoded.currentBlock,
        decoded.deadline + U256::from(1),
        "DeadlineExceeded fired at block {} but the deadline was {} — expected the very next \
         block (deadline + 1)",
        decoded.currentBlock,
        decoded.deadline,
    );
}
