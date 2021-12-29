//! Dry run settlement submission strategy. I.e. just log!

use crate::{
    settlement::Settlement, settlement_simulation::settle_method_builder,
    settlement_simulation::tenderly_link,
};
use anyhow::Result;
use contracts::GPv2Settlement;
use ethcontract::Account;
use web3::types::TransactionReceipt;

pub async fn log_settlement(
    account: Account,
    contract: &GPv2Settlement,
    settlement: Settlement,
) -> Result<TransactionReceipt> {
    let web3 = contract.raw_instance().web3();
    let current_block = web3.eth().block_number().await?;
    let network = web3.net().version().await?;
    let settlement = settle_method_builder(contract, settlement.into(), account).tx;
    let simulation_link = tenderly_link(current_block.as_u64(), &network, settlement);

    tracing::info!("not submitting transaction in dry-run mode");
    tracing::debug!("transaction simulation: {}", simulation_link);

    // We could technically compute a transaction hash for the settlement here,
    // but it's probably not worth the effort.
    Ok(TransactionReceipt {
        transaction_hash: Default::default(),
        block_hash: Some(Default::default()),
        block_number: Some(Default::default()),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::H160;
    use ethcontract_mock::Mock;

    #[tokio::test]
    async fn queries_and_logs_settlement() {
        let mock = Mock::new(42);
        let web3 = mock.web3();

        assert!(log_settlement(
            Account::Local(H160([2; 20]), None),
            &GPv2Settlement::at(&web3, H160([1; 20])),
            Settlement::new(Default::default()),
        )
        .await
        .is_ok());
    }
}
