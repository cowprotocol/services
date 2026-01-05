//! EIP-1559 transaction submitter for EthFlow refunds.
//!
//! Submits transactions with a small priority tip but high `max_fee_per_gas`
//! for fast inclusion. If a transaction is not mined within 5 blocks, the
//! caller should retry. When retrying with the same nonce, gas parameters are
//! automatically bumped by 12.5% to avoid "replacement underpriced" errors.

use {
    crate::traits::ChainWrite,
    alloy::{primitives::Address, providers::Provider},
    anyhow::{Context, Result},
    contracts::alloy::CoWSwapEthFlow::{self, EthFlowOrder},
    database::OrderUid,
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    shared::ethrpc::Web3,
    std::time::Duration,
};

/// Buffer factor applied to gas price estimates (1.3x = 30% buffer).
const GAS_PRICE_BUFFER_FACTOR: f64 = 1.3;

/// Minimum bump required for transaction replacement (1.125x = 12.5% increase).
const GAS_PRICE_BUMP: f64 = 1.125;

/// Truncates `f64` to `u128`. Safe here because gas values are always integral.
const fn f64_to_u128(n: f64) -> u128 {
    n as u128
}

/// Manages EIP-1559 transaction submission with automatic gas price escalation.
pub struct Submitter {
    pub web3: Web3,
    pub signer_address: Address,
    pub gas_estimator: Box<dyn GasPriceEstimating>,
    pub gas_parameters_of_last_tx: Option<GasPrice1559>,
    pub nonce_of_last_submission: Option<u64>,
    pub max_gas_price: u64,
    pub start_priority_fee_tip: u64,
}

impl Submitter {
    /// Fetches the on-chain transaction count for the signer address.
    ///
    /// Returns the on-chain transaction count, used as the nonce for the next
    /// submission. Does not include pending mempool transactions.
    async fn get_submission_nonce(&self) -> Result<u64> {
        self.web3
            .alloy
            .get_transaction_count(self.signer_address)
            .await
            .with_context(|| {
                format!(
                    "could not get latest nonce for address {:?}",
                    self.signer_address
                )
            })
    }

    /// Submits a batch refund transaction to the EthFlow contract.
    ///
    /// Calls `invalidateOrdersIgnoringNotAllowed()` with the given orders.
    ///
    /// # Errors
    ///
    /// Returns an error if nonce retrieval or gas estimation fails.
    /// Transaction submission or on-chain execution failures are logged
    /// but do not return an error (allows the loop to continue).
    async fn submit(
        &mut self,
        uids: &[OrderUid],
        encoded_ethflow_orders: Vec<EthFlowOrder::Data>,
        ethflow_contract: Address,
    ) -> Result<()> {
        const TIMEOUT_5_BLOCKS: Duration = Duration::from_secs(60);

        let gas_price_estimation = self.gas_estimator.estimate().await?;
        let nonce = self.get_submission_nonce().await?;
        let gas_price = calculate_submission_gas_price(
            self.gas_parameters_of_last_tx,
            gas_price_estimation,
            nonce,
            self.nonce_of_last_submission,
            self.max_gas_price,
            self.start_priority_fee_tip,
        )?;

        self.gas_parameters_of_last_tx = Some(gas_price);
        self.nonce_of_last_submission = Some(nonce);

        let ethflow_contract =
            CoWSwapEthFlow::Instance::new(ethflow_contract, self.web3.alloy.clone());
        let tx_result = ethflow_contract
            .invalidateOrdersIgnoringNotAllowed(encoded_ethflow_orders)
            // Gas values are integral; truncation should be safe even though they're floats
            .max_priority_fee_per_gas(f64_to_u128(gas_price.max_priority_fee_per_gas))
            .max_fee_per_gas(f64_to_u128(gas_price.max_fee_per_gas))
            .from(self.signer_address)
            .nonce(nonce)
            .send()
            .await?.with_timeout(Some(TIMEOUT_5_BLOCKS)).get_receipt().await;

        match tx_result {
            Ok(receipt) => {
                tracing::debug!(
                    "Tx to refund the orderuids {:?} yielded following result {:?}",
                    uids,
                    receipt
                );
            }
            Err(err) => tracing::debug!("transaction failed with: {err}"),
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ChainWrite for Submitter {
    async fn submit_refund(
        &mut self,
        uids: &[OrderUid],
        encoded_ethflow_orders: Vec<EthFlowOrder::Data>,
        ethflow_contract: Address,
    ) -> Result<()> {
        self.submit(uids, encoded_ethflow_orders, ethflow_contract)
            .await
    }
}

/// Calculates EIP-1559 gas parameters for transaction submission.
///
/// Applies the buffer factor to the estimated gas price, caps the priority fee,
/// bumps parameters by 12.5% when replacing a pending transaction (same nonce),
/// and caps the result at `max_gas_price`.
fn calculate_submission_gas_price(
    gas_price_of_last_submission: Option<GasPrice1559>,
    web3_gas_estimation: GasPrice1559,
    newest_nonce: u64,
    nonce_of_last_submission: Option<u64>,
    max_gas_price: u64,
    start_priority_fee_tip: u64,
) -> Result<GasPrice1559> {
    // Start with the current gas estimate plus a buffer for faster inclusion.
    let mut new_gas_price = web3_gas_estimation.bump(GAS_PRICE_BUFFER_FACTOR);
    // Cap priority fee at max_fee_per_gas; required for valid EIP-1559
    // transactions.
    new_gas_price.max_priority_fee_per_gas =
        (start_priority_fee_tip as f64).min(new_gas_price.max_fee_per_gas);

    // If the previous submission was not mined (same nonce), bump gas parameters
    // by 12.5% to avoid "replacement transaction underpriced" errors.
    if Some(newest_nonce) == nonce_of_last_submission
        && let Some(gas_price_of_last_submission) = gas_price_of_last_submission
    {
        let gas_price_of_last_submission = gas_price_of_last_submission.bump(GAS_PRICE_BUMP);
        new_gas_price.max_fee_per_gas = new_gas_price
            .max_fee_per_gas
            .max(gas_price_of_last_submission.max_fee_per_gas);
        new_gas_price.max_priority_fee_per_gas = new_gas_price
            .max_priority_fee_per_gas
            .max(gas_price_of_last_submission.max_priority_fee_per_gas);
    }

    if new_gas_price.max_fee_per_gas > max_gas_price as f64 {
        tracing::warn!(
            "Refunding txs are likely not mined in time, as the current gas price {:?} is higher \
             than MAX_GAS_PRICE specified {:?}",
            new_gas_price.max_fee_per_gas,
            max_gas_price
        );
        new_gas_price.max_fee_per_gas =
            f64::min(max_gas_price as f64, new_gas_price.max_fee_per_gas);
    }
    new_gas_price.max_priority_fee_per_gas = f64::min(
        new_gas_price.max_priority_fee_per_gas,
        new_gas_price.max_fee_per_gas,
    );
    Ok(new_gas_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_submission_gas_price() {
        const TEST_MAX_GAS_PRICE: u64 = 800_000_000_000;
        const TEST_START_PRIORITY_FEE_TIP: u64 = 2_000_000_000;

        // First case: previous tx was successful
        let max_fee_per_gas = 4_000_000_000f64;
        let web3_gas_estimation = GasPrice1559 {
            base_fee_per_gas: 2_000_000_000f64,
            max_fee_per_gas,
            max_priority_fee_per_gas: 3_000_000_000f64,
        };
        let newest_nonce = 1;
        let nonce_of_last_submission = None;
        let gas_price_of_last_submission = None;
        let result = calculate_submission_gas_price(
            gas_price_of_last_submission,
            web3_gas_estimation,
            newest_nonce,
            nonce_of_last_submission,
            TEST_MAX_GAS_PRICE,
            TEST_START_PRIORITY_FEE_TIP,
        )
        .unwrap();
        let expected_result = GasPrice1559 {
            max_fee_per_gas: max_fee_per_gas * GAS_PRICE_BUFFER_FACTOR,
            max_priority_fee_per_gas: TEST_START_PRIORITY_FEE_TIP as f64,
            base_fee_per_gas: 2_000_000_000f64,
        };
        assert_eq!(result, expected_result);
        // Second case: Previous tx was not successful
        let nonce_of_last_submission = Some(newest_nonce);
        let max_fee_per_gas_of_last_tx = max_fee_per_gas * 2f64;
        let gas_price_of_last_submission = GasPrice1559 {
            max_fee_per_gas: max_fee_per_gas_of_last_tx,
            max_priority_fee_per_gas: TEST_START_PRIORITY_FEE_TIP as f64,
            base_fee_per_gas: 2_000_000_000f64,
        };
        let result = calculate_submission_gas_price(
            Some(gas_price_of_last_submission),
            web3_gas_estimation,
            newest_nonce,
            nonce_of_last_submission,
            TEST_MAX_GAS_PRICE,
            TEST_START_PRIORITY_FEE_TIP,
        )
        .unwrap();
        let expected_result = GasPrice1559 {
            max_fee_per_gas: max_fee_per_gas_of_last_tx * GAS_PRICE_BUMP,
            max_priority_fee_per_gas: TEST_START_PRIORITY_FEE_TIP as f64 * GAS_PRICE_BUMP,
            base_fee_per_gas: 2_000_000_000f64,
        };
        assert_eq!(result, expected_result);
        // Third case: MAX_GAS_PRICE is not exceeded
        let max_fee_per_gas = TEST_MAX_GAS_PRICE as f64 + 1000f64;
        let web3_gas_estimation = GasPrice1559 {
            base_fee_per_gas: 2_000_000_000f64,
            max_fee_per_gas,
            max_priority_fee_per_gas: 3_000_000_000f64,
        };
        let nonce_of_last_submission = None;
        let gas_price_of_last_submission = None;
        let result = calculate_submission_gas_price(
            gas_price_of_last_submission,
            web3_gas_estimation,
            newest_nonce,
            nonce_of_last_submission,
            TEST_MAX_GAS_PRICE,
            TEST_START_PRIORITY_FEE_TIP,
        )
        .unwrap();
        let expected_result = GasPrice1559 {
            base_fee_per_gas: 2_000_000_000f64,
            max_fee_per_gas: TEST_MAX_GAS_PRICE as f64,
            max_priority_fee_per_gas: TEST_START_PRIORITY_FEE_TIP as f64,
        };
        assert_eq!(result, expected_result);
    }

    /// Test that when a nonce conflict occurs (on-chain nonce advances
    /// unexpectedly, e.g., due to a manual transaction), the gas price bump
    /// is NOT applied.
    ///
    /// This verifies that the submitter correctly detects when the nonce has
    /// changed and doesn't apply the 12.5% replacement bump in that case.
    #[test]
    fn test_nonce_conflict_doesnt_apply_gas_bump() {
        const TEST_MAX_GAS_PRICE: u64 = 800_000_000_000;
        const TEST_START_PRIORITY_FEE_TIP: u64 = 2_000_000_000;

        // Previous submission attempted with nonce 5
        let nonce_of_last_submission = Some(5);
        let gas_price_of_last_submission = GasPrice1559 {
            max_fee_per_gas: 100_000_000_000f64,
            max_priority_fee_per_gas: 10_000_000_000f64,
            base_fee_per_gas: 50_000_000_000f64,
        };

        // Current network gas estimate
        let web3_gas_estimation = GasPrice1559 {
            base_fee_per_gas: 40_000_000_000f64,
            max_fee_per_gas: 80_000_000_000f64,
            max_priority_fee_per_gas: 8_000_000_000f64,
        };

        // BUT: on-chain nonce has advanced to 7 (someone else submitted a tx)
        let newest_nonce = 7;

        let result = calculate_submission_gas_price(
            Some(gas_price_of_last_submission),
            web3_gas_estimation,
            newest_nonce,
            nonce_of_last_submission,
            TEST_MAX_GAS_PRICE,
            TEST_START_PRIORITY_FEE_TIP,
        )
        .unwrap();

        // Expected: NO bump applied because nonce changed (7 != 5)
        // Result should be based on web3_gas_estimation * GAS_PRICE_BUFFER_FACTOR
        let expected = GasPrice1559 {
            base_fee_per_gas: 40_000_000_000f64,
            max_fee_per_gas: 80_000_000_000f64 * GAS_PRICE_BUFFER_FACTOR,
            max_priority_fee_per_gas: TEST_START_PRIORITY_FEE_TIP as f64,
        };

        assert_eq!(
            result.max_fee_per_gas, expected.max_fee_per_gas,
            "Max fee per gas should be buffered estimate, not bumped"
        );
        assert_eq!(
            result.max_priority_fee_per_gas, expected.max_priority_fee_per_gas,
            "Priority fee should be capped at start tip, not bumped"
        );

        // Verify that the result is NOT the bumped version
        let bumped_gas_price = gas_price_of_last_submission.bump(GAS_PRICE_BUMP);
        assert_ne!(
            result.max_fee_per_gas, bumped_gas_price.max_fee_per_gas,
            "Gas price should NOT be bumped when nonce changed"
        );
    }
}
