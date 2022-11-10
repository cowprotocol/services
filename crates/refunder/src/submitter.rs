// This submitter has the follwoing logic:
// It tries to submit a tx - as EIP1559 - with a small tx tip,
// but a quite high max_fee_per_gas such that it's likely being mined quickly
//
// Then it waits for 5 blocks. If the tx is not mined, it will return an error
// and it needs to be called again. If the last submission was not successful,
// this submitter stores the last gas_price in order to submit the new tx with
// a higher gas price, in order to avoid: ErrReplaceUnderpriced erros
// In the re-newed attempt for submission the same nonce is used as before.

use crate::refund_service::EncodedEthflowOrder;
use anyhow::{anyhow, Result};
use contracts::CoWSwapEthFlow;
use database::OrderUid;
use ethcontract::{
    transaction::{confirm::ConfirmParams, ResolveCondition},
    Account, GasPrice, U256,
};
use gas_estimation::{GasPrice1559, GasPriceEstimating};
use shared::{
    ethrpc::Web3,
    submitter_constants::{TX_ALREADY_KNOWN, TX_ALREADY_MINED},
};

// Max gas used for submitting transactions
// If the gas price is higher than this value,
// the service will temproarily not refund users
const MAX_GAS_PRICE: u64 = 500_000_000_000u64;

// The gas price buffer determine the gas price buffer used to
// send out EIP1559 txs.
// Example: If the prevailing gas is 10Gwei and the buffer is 20
// then the gas_price used will be 12.
const GAS_PRICE_BUFFER_IN_PERCENT: f64 = 30.0f64;
// Max priority fee that the refunder is willing to pay. (=2 Gwei)
const PRIORITY_TIP_OF_TRANSACTION: u64 = 2_000_000_000u64;
// In order to resubmit a new tx with the same nonce, the gas price needs to
// be increased by at least 10 percent. We increase it by 12 percent
const GAS_PRICE_BUMP: f64 = 1.12f64;

pub struct Submitter {
    pub web3: Web3,
    pub ethflow_contract: CoWSwapEthFlow,
    pub account: Account,
    pub gas_estimator: Box<dyn GasPriceEstimating>,
    pub gas_price_of_last_submission: Option<U256>,
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
            self.gas_price_of_last_submission,
            gas_price_estimation,
            nonce,
            self.nonce_of_last_submission,
        )?;
        let GasPrice::Eip1559 { max_fee_per_gas, ..} = gas_price else {
            return Err(anyhow!("Unreachable state during refunder submission"));
        };

        // Gas prices are capped at MAX_GAS_PRICE
        if max_fee_per_gas < U256::from(MAX_GAS_PRICE) {
            self.gas_price_of_last_submission = Some(max_fee_per_gas);
            self.nonce_of_last_submission = Some(nonce);
            let tx_result = self
                .ethflow_contract
                .invalidate_orders_ignoring_not_allowed(encoded_ethflow_orders)
                .gas_price(gas_price)
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
                        // This can only happen after restarts, as usually we would always increase
                        // the gas price compared to previous tx.
                        // In this situation, we restart the submission loop with double the
                        // previous gas. Due to the nature of EIP1559 this will not increase the
                        // cost of the tx
                        // todo: Finding last tx and its gas price would be nicer
                        self.gas_price_of_last_submission = Some(max_fee_per_gas * 2);
                        tracing::debug!(?err, "transaction already known");
                    } else {
                        tracing::warn!(?err, "submission failed");
                    }
                }
            }
        } else {
            tracing::warn!(
                "Refund tx are not started, as the current gas price {:?} \
                            is higher than MAX_GAS_PRICE specified {:?}",
                max_fee_per_gas,
                MAX_GAS_PRICE
            );
        }
        Ok(())
    }
}

fn calculate_submission_gas_price(
    gas_price_of_last_submission: Option<U256>,
    web3_gas_estimation: GasPrice1559,
    newest_nonce: U256,
    nonce_of_last_submission: Option<U256>,
) -> Result<GasPrice> {
    let GasPrice1559 {
        max_fee_per_gas, ..
    }: gas_estimation::GasPrice1559 = web3_gas_estimation;
    // The gas price of the refund tx is the current prevailing gas price
    // of the web3 gas estimation plus a buffer.
    // Since we are using Eip1559 gas specification,
    // we will only pay the buffer if it is used
    let mut new_max_fee_per_gas = max_fee_per_gas * (1f64 + GAS_PRICE_BUFFER_IN_PERCENT / 100f64);
    // If tx from the previous submission was not mined,
    // we incease the gas price
    if let Some(nonce_of_last_submission) = nonce_of_last_submission {
        if nonce_of_last_submission == newest_nonce {
            // Increase the gas price to last submission's
            // gas price plus an additional 12 %
            if let Some(previous_gas_price) = gas_price_of_last_submission {
                new_max_fee_per_gas = f64::max(
                    new_max_fee_per_gas,
                    previous_gas_price.to_f64_lossy() * GAS_PRICE_BUMP,
                );
            }
        }
    }
    let gas_price = GasPrice::Eip1559 {
        max_fee_per_gas: U256::from(new_max_fee_per_gas as u64),
        max_priority_fee_per_gas: U256::from(PRIORITY_TIP_OF_TRANSACTION),
    };
    Ok(gas_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_submission_gas_price() {
        // First case: previous tx was successful
        let max_fee_per_gas = 10f64;
        let web3_gas_estimation = GasPrice1559 {
            base_fee_per_gas: 2f64,
            max_fee_per_gas,
            max_priority_fee_per_gas: 10f64,
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
        let expected_result = GasPrice::Eip1559 {
            max_fee_per_gas: U256::from(
                (max_fee_per_gas * (1f64 + GAS_PRICE_BUFFER_IN_PERCENT / 100f64)) as u64,
            ),
            max_priority_fee_per_gas: U256::from(PRIORITY_TIP_OF_TRANSACTION),
        };
        assert_eq!(result, expected_result);
        // Second case: Preivous tx was not successful
        let nonce_of_last_submission = Some(newest_nonce.clone());
        let gas_price_of_last_submission = 100f64;
        let result = calculate_submission_gas_price(
            Some(U256::from(gas_price_of_last_submission as u64)),
            web3_gas_estimation,
            newest_nonce,
            nonce_of_last_submission,
        )
        .unwrap();
        let expected_result = GasPrice::Eip1559 {
            max_fee_per_gas: U256::from((gas_price_of_last_submission * GAS_PRICE_BUMP) as u64),
            max_priority_fee_per_gas: U256::from(PRIORITY_TIP_OF_TRANSACTION),
        };
        assert_eq!(result, expected_result);
    }
}
