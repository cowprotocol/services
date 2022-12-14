// This submitter has the following logic:
// It tries to submit a tx - as EIP1559 - with a small tx tip,
// but a quite high max_fee_per_gas such that it's likely being mined quickly
//
// Then it waits for 5 blocks. If the tx is not mined, it will return an error
// and it needs to be called again. If the last submission was not successful,
// this submitter stores the last gas_price in order to submit the new tx with
// a higher gas price, in order to avoid: ErrReplaceUnderpriced erros
// In the re-newed attempt for submission the same nonce is used as before.

use super::ethflow_order::EncodedEthflowOrder;
use anyhow::{anyhow, Result};
use contracts::CoWSwapEthFlow;
use database::OrderUid;
use ethcontract::{
    transaction::{confirm::ConfirmParams, ResolveCondition},
    Account, U256,
};
use gas_estimation::{GasPrice1559, GasPriceEstimating};
use shared::{
    conversions::into_gas_price,
    ethrpc::Web3,
    submitter_constants::{TX_ALREADY_KNOWN, TX_ALREADY_MINED},
};

// Max gas price used for submitting transactions
const MAX_GAS_PRICE: u64 = 800_000_000_000;

// The gas price buffer determines the gas price buffer used to
// send out EIP1559 txs.
// Example: If the prevailing gas is 10Gwei and the buffer factor is 1.20
// then the gas_price used will be 12.
const GAS_PRICE_BUFFER_FACTOR: f64 = 1.3;
// Starting priority fee that the refunder is willing to pay. (=2 Gwei)
const START_PRIORITY_FEE_TIP: u64 = 2_000_000_000;

// In order to resubmit a new tx with the same nonce, the gas tip and
// max_fee_per_gas needs to be increased by at least 10 percent.
const GAS_PRICE_BUMP: f64 = 1.125;

pub struct Submitter {
    pub web3: Web3,
    pub ethflow_contract: CoWSwapEthFlow,
    pub account: Account,
    pub gas_estimator: Box<dyn GasPriceEstimating>,
    pub gas_parameters_of_last_tx: Option<GasPrice1559>,
    pub nonce_of_last_submission: Option<U256>,
}

impl Submitter {
    async fn get_submission_nonce(&self) -> Result<U256> {
        // this command returns the tx count ever mined at the latest block
        // Mempool tx are not considered.
        self.web3
            .eth()
            .transaction_count(self.account.address(), None)
            .await
            .map_err(|err| anyhow!("Could not get latest nonce due to err: {:}", err))
    }

    pub async fn submit(
        &mut self,
        uids: Vec<OrderUid>,
        encoded_ethflow_orders: Vec<EncodedEthflowOrder>,
    ) -> Result<()> {
        let confirm_params = ConfirmParams {
            block_timeout: Some(5),
            ..Default::default()
        };
        let resolve_conditions = ResolveCondition::Confirmed(confirm_params);
        let gas_price_estimation = self.gas_estimator.estimate().await?;
        let nonce = self.get_submission_nonce().await?;
        let gas_price = calculate_submission_gas_price(
            self.gas_parameters_of_last_tx,
            gas_price_estimation,
            nonce,
            self.nonce_of_last_submission,
        )?;

        self.gas_parameters_of_last_tx = Some(gas_price);
        self.nonce_of_last_submission = Some(nonce);
        let tx_result = self
            .ethflow_contract
            .invalidate_orders_ignoring_not_allowed(encoded_ethflow_orders)
            .gas_price(into_gas_price(&gas_price))
            .from(self.account.clone())
            .nonce(nonce)
            .into_inner()
            .resolve(resolve_conditions)
            .send()
            .await;
        match tx_result {
            Ok(handle) => {
                tracing::debug!(
                    "Tx to refund the orderuids {:?} yielded following result {:?}",
                    uids,
                    handle
                );
            }
            Err(err) => {
                let err = err.to_string();
                if TX_ALREADY_MINED.iter().any(|msg| err.contains(msg)) {
                    // It could happen that the previous tx got mined right before the tx was
                    // send.
                    tracing::debug!(?err, "transaction already mined");
                } else if TX_ALREADY_KNOWN.iter().any(|msg| err.contains(msg)) {
                    // This case means that the node is already aware of the tx
                    // This can only happen after restarts, or close to the MAX_GAS_PRICE
                    // as usually we would always increase the gas tip compared to previous tx.
                    // Hence, we irgnore the warning and just retry.
                    tracing::debug!(?err, "transaction already known");
                } else {
                    // Todo: Handle the error "replacement transaction underpriced"
                    // This could happen after restarts or close to the MAX_GAS_PRICE
                    tracing::warn!(?err, "submission failed");
                }
            }
        }
        Ok(())
    }
}

fn calculate_submission_gas_price(
    gas_price_of_last_submission: Option<GasPrice1559>,
    web3_gas_estimation: GasPrice1559,
    newest_nonce: U256,
    nonce_of_last_submission: Option<U256>,
) -> Result<GasPrice1559> {
    // The gas price of the refund tx is the current prevailing gas price
    // of the web3 gas estimation plus a buffer.
    let mut new_gas_price = web3_gas_estimation.bump(GAS_PRICE_BUFFER_FACTOR);
    // limit the prio_fee to max_fee_per_gas as otherwise tx is invalid
    new_gas_price.max_priority_fee_per_gas =
        (START_PRIORITY_FEE_TIP as f64).min(new_gas_price.max_fee_per_gas);

    // If tx from the previous submission was not mined,
    // we incease the tip and max_gas_fee for miners
    // in order to avoid "tx underpriced errors"
    if Some(newest_nonce) == nonce_of_last_submission {
        if let Some(gas_price_of_last_submission) = gas_price_of_last_submission {
            let gas_price_of_last_submission = gas_price_of_last_submission.bump(GAS_PRICE_BUMP);
            new_gas_price.max_fee_per_gas = new_gas_price
                .max_fee_per_gas
                .max(gas_price_of_last_submission.max_fee_per_gas);
            new_gas_price.max_priority_fee_per_gas = new_gas_price
                .max_priority_fee_per_gas
                .max(gas_price_of_last_submission.max_priority_fee_per_gas);
        }
    }

    if new_gas_price.max_fee_per_gas > MAX_GAS_PRICE as f64 {
        tracing::warn!(
            "Refunding txs are likely not mined in time, as the current gas price {:?} \
                            is higher than MAX_GAS_PRICE specified {:?}",
            new_gas_price.max_fee_per_gas,
            MAX_GAS_PRICE
        );
        new_gas_price.max_fee_per_gas =
            f64::min(MAX_GAS_PRICE as f64, new_gas_price.max_fee_per_gas);
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
        // First case: previous tx was successful
        let max_fee_per_gas = 4_000_000_000f64;
        let web3_gas_estimation = GasPrice1559 {
            base_fee_per_gas: 2_000_000_000f64,
            max_fee_per_gas,
            max_priority_fee_per_gas: 3_000_000_000f64,
        };
        let newest_nonce = U256::one();
        let nonce_of_last_submission = None;
        let gas_price_of_last_submission = None;
        let result = calculate_submission_gas_price(
            gas_price_of_last_submission,
            web3_gas_estimation,
            newest_nonce,
            nonce_of_last_submission,
        )
        .unwrap();
        let expected_result = GasPrice1559 {
            max_fee_per_gas: max_fee_per_gas * GAS_PRICE_BUFFER_FACTOR,
            max_priority_fee_per_gas: START_PRIORITY_FEE_TIP as f64,
            base_fee_per_gas: 2_000_000_000f64,
        };
        assert_eq!(result, expected_result);
        // Second case: Previous tx was not successful
        let nonce_of_last_submission = Some(newest_nonce);
        let max_fee_per_gas_of_last_tx = max_fee_per_gas * 2f64;
        let gas_price_of_last_submission = GasPrice1559 {
            max_fee_per_gas: max_fee_per_gas_of_last_tx,
            max_priority_fee_per_gas: START_PRIORITY_FEE_TIP as f64,
            base_fee_per_gas: 2_000_000_000f64,
        };
        let result = calculate_submission_gas_price(
            Some(gas_price_of_last_submission),
            web3_gas_estimation,
            newest_nonce,
            nonce_of_last_submission,
        )
        .unwrap();
        let expected_result = GasPrice1559 {
            max_fee_per_gas: max_fee_per_gas_of_last_tx * GAS_PRICE_BUMP,
            max_priority_fee_per_gas: START_PRIORITY_FEE_TIP as f64 * GAS_PRICE_BUMP,
            base_fee_per_gas: 2_000_000_000f64,
        };
        assert_eq!(result, expected_result);
        // Thrid case: MAX_GAS_PRICE is not exceeded
        let max_fee_per_gas = MAX_GAS_PRICE as f64 + 1000f64;
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
        )
        .unwrap();
        let expected_result = GasPrice1559 {
            base_fee_per_gas: 2_000_000_000f64,
            max_fee_per_gas: MAX_GAS_PRICE as f64,
            max_priority_fee_per_gas: START_PRIORITY_FEE_TIP as f64,
        };
        assert_eq!(result, expected_result);
    }
}
