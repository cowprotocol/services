use {
    crate::tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution},
    },
    alloy::{consensus::Transaction, providers::Provider},
};

const MAX_FEE_PER_GAS: u128 = 100_000_000_000;
const MAX_PRIORITY_FEE_PER_GAS: u128 = 2_000_000_000;

/// Verify that a solution with custom gas fee overrides settles successfully
/// and the overrides are applied to the on-chain transaction.
#[tokio::test]
#[ignore]
async fn settle_with_gas_fee_override() {
    let test = tests::setup()
        .name("gas fee override")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution().gas_fee_override(MAX_FEE_PER_GAS, MAX_PRIORITY_FEE_PER_GAS))
        .done()
        .await;

    let id = test.solve().await.ok().id();
    test.settle(id)
        .await
        .ok()
        .await
        .ab_order_executed(&test)
        .await;

    // Verify the settlement transaction used the solver's gas fee override.
    let block = test
        .web3()
        .provider
        .get_block_by_number(Default::default())
        .await
        .unwrap()
        .unwrap();
    let tx_hash = block.transactions.hashes().next().unwrap();
    let tx = test
        .web3()
        .provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();
    let consensus_tx: &dyn Transaction = &*tx.inner;
    assert_eq!(
        consensus_tx.max_fee_per_gas(),
        MAX_FEE_PER_GAS,
        "settlement tx should use the solver's maxFeePerGas override"
    );
    assert_eq!(
        consensus_tx.max_priority_fee_per_gas(),
        Some(MAX_PRIORITY_FEE_PER_GAS),
        "settlement tx should use the solver's maxPriorityFeePerGas override"
    );
}
