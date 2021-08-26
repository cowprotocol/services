// Design:
// As in the traditional transaction submission workflow the main work in this module is checking
// the gas price in a loop and updating the transaction when the gas price increases. This differs
// so that we can make use of the property that archer transactions do not cost gas if they fail.
// When we detect that the transaction would no longer succeed we stop trying to submit and return
// so that the solver can run again.
// In addition to simulation failure we make use of a deadline after which submission attempts also
// stop. This allows the solver to update and improve a solution even if it hasn't yet become
// invalid.
// We do not know in advance which of our submitted transactions will get mined. Instead of polling
// all of them we only check the account's nonce as an optimization. When this happens all our
// transactions definitely become invalid (even if the transaction came for whatever reason
// from outside) so it is only at that point that we need to check the hashes individually to the
// find the one that got mined (if any).

// Idea:
// The current driver code is either solving or waiting for a solution to be mined.
// When using archer we can improve on this by always solving the current order book as if there was
// no pending transaction and continually updating the transaction we are sending to archer.
// This is a bigger change so the code here still adheres to the traditional tx submission
// workflow.

// TODO: Make sure settlement contract always has eth to pay the miner. Could reuse buffers or just
// manually fund it like we do with other solver accounts and make part of our monitor scripts.

// TODO: Node gas estimates seem a little higher than actual. This can be seen when comparing
// to tenderly simulations and mined transactions. This causes us to overpay somewhat.

use super::{archer_api::ArcherApi, ESTIMATE_GAS_LIMIT_FACTOR};
use crate::{interactions::block_coinbase, settlement::Settlement};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use contracts::GPv2Settlement;
use ethcontract::{errors::MethodError, transaction::Transaction, Account, GasPrice};
use futures::FutureExt;
use gas_estimation::GasPriceEstimating;
use primitive_types::{H256, U256};
use shared::Web3;
use std::time::{Duration, Instant, SystemTime};
use web3::types::TransactionId;

pub struct ArcherSolutionSubmitter<'a> {
    pub web3: &'a Web3,
    pub contract: &'a GPv2Settlement,
    pub account: &'a Account,
    pub archer_api: &'a ArcherApi,
    pub gas_price_estimator: &'a dyn GasPriceEstimating,
    pub gas_price_cap: f64,
}

impl<'a> ArcherSolutionSubmitter<'a> {
    /// Submit a settlement to the contract, updating the transaction with gas prices if they increase.
    ///
    /// Goes through the archerdao network so that failing transactions do not get mined and thus do
    /// not cost gas.
    ///
    /// Returns None if the deadline is reached without a mined transaction.
    ///
    /// Only works on mainnet.
    pub async fn submit(
        &self,
        target_confirm_time: Duration,
        deadline: SystemTime,
        settlement: Settlement,
        gas_estimate: U256,
    ) -> Result<Option<H256>> {
        let nonce = self.nonce().await?;

        tracing::info!(
            "starting archer solution submission at nonce {} with deadline {}",
            nonce,
            DateTime::<Utc>::from(deadline).to_rfc3339(),
        );

        let mut transactions = Vec::new();
        let submit_future = self.submit_with_increasing_gas_prices_until_simulation_fails(
            deadline,
            target_confirm_time,
            nonce,
            settlement,
            gas_estimate,
            &mut transactions,
        );

        let nonce_future = self.wait_for_nonce_to_change(nonce);

        let deadline_future = tokio::time::sleep(
            deadline
                .duration_since(SystemTime::now())
                .unwrap_or_else(|_| Duration::from_secs(0)),
        );

        futures::select! {
            method_error = submit_future.fuse() => tracing::info!("stopping submission because simulation failed: {:?}", method_error),
            new_nonce = nonce_future.fuse() => tracing::info!("stopping submission because account nonce changed to {}", new_nonce),
            _ = deadline_future.fuse() => tracing::info!("stopping submission because deadline has been reached"),
        };

        // After stopping submission of new transactions we wait for some time to give a potentially
        // mined previously submitted transaction time to propagate to our node.

        if !transactions.is_empty() {
            const MINED_TX_PROPAGATE_TIME: Duration = Duration::from_secs(20);
            const MINED_TX_CHECK_INTERVAL: Duration = Duration::from_secs(5);
            let tx_to_propagate_deadline = Instant::now() + MINED_TX_PROPAGATE_TIME;

            tracing::info!(
                "waiting up to {} seconds to see if a transaction was mined",
                MINED_TX_PROPAGATE_TIME.as_secs()
            );

            loop {
                if let Some(hash) = find_mined_transaction(self.web3, &transactions).await {
                    tracing::info!("found mined transaction {}", hash);
                    return Ok(Some(hash));
                }
                if Instant::now() + MINED_TX_CHECK_INTERVAL > tx_to_propagate_deadline {
                    break;
                }
                tokio::time::sleep(MINED_TX_CHECK_INTERVAL).await;
            }
        }

        tracing::info!("did not find any mined transaction");
        Ok(None)
    }

    async fn nonce(&self) -> Result<U256> {
        self.web3
            .eth()
            .transaction_count(self.account.address(), None)
            .await
            .context("transaction_count")
    }

    /// Keep polling the account's nonce until it is different from initial_nonce returning the new
    /// nonce.
    async fn wait_for_nonce_to_change(&self, initial_nonce: U256) -> U256 {
        const POLL_INTERVAL: Duration = Duration::from_secs(1);
        loop {
            match self.nonce().await {
                Ok(nonce) if nonce != initial_nonce => return nonce,
                Ok(_) => (),
                Err(err) => tracing::error!("web3 error while getting nonce: {:?}", err),
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    async fn gas_price(&self, gas_limit: f64, time_limit: Duration) -> Result<f64> {
        match self
            .gas_price_estimator
            .estimate_with_limits(gas_limit, time_limit)
            .await
        {
            Ok(gas_price) if gas_price <= self.gas_price_cap => Ok(gas_price),
            Ok(gas_price) => Err(anyhow!(
                "gas station gas price {} is larger than cap {}",
                gas_price,
                self.gas_price_cap
            )),
            Err(err) => Err(err),
        }
    }

    /// Keep submitting the settlement transaction to the archer network as gas price changes.
    ///
    /// Returns when simulation of the transaction fails. This likely happens if the settlement
    /// becomes invalid due to changing prices or the account's nonce changes.
    ///
    /// Potential transaction hashes are communicated back through a shared vector.
    async fn submit_with_increasing_gas_prices_until_simulation_fails(
        &self,
        deadline: SystemTime,
        target_confirm_time: Duration,
        nonce: U256,
        settlement: Settlement,
        gas_estimate: U256,
        transactions: &mut Vec<H256>,
    ) -> MethodError {
        const UPDATE_INTERVAL: Duration = Duration::from_secs(10);

        // The amount of extra gas it costs to include the payment to block.coinbase interaction in
        // an existing settlement.
        let gas_estimate = gas_estimate + U256::from(18346);
        let target_confirm_time = Instant::now() + target_confirm_time;

        // gas price and raw signed transaction
        let mut previous_tx: Option<(f64, Vec<u8>)> = None;

        loop {
            // get gas price
            // Account for some buffer in the gas limit in case racing state changes result in slightly more heavy computation at execution time.
            let gas_limit = gas_estimate.to_f64_lossy() * ESTIMATE_GAS_LIMIT_FACTOR;
            let time_limit = target_confirm_time.saturating_duration_since(Instant::now());
            let gas_price = match self.gas_price(gas_limit, time_limit).await {
                Ok(gas_price) => gas_price,
                Err(err) => {
                    tracing::error!("gas estimation failed: {:?}", err);
                    tokio::time::sleep(UPDATE_INTERVAL).await;
                    continue;
                }
            };

            // create transaction

            let tx_gas_cost_in_ether_wei = U256::from_f64_lossy(gas_price) * gas_estimate;
            let mut settlement = settlement.clone();
            settlement
                .encoder
                .append_to_execution_plan(block_coinbase::PayBlockCoinbase {
                    amount: tx_gas_cost_in_ether_wei,
                });
            let method = super::retry::settle_method_builder(
                self.contract,
                settlement.into(),
                self.account.clone(),
            )
            .nonce(nonce)
            // Wouldn't work because the function isn't payable.
            // .value(tx_gas_cost_in_ether_wei)
            .gas(U256::from_f64_lossy(gas_limit))
            .gas_price(GasPrice::Value(U256::zero()));

            // simulate transaction

            if let Err(err) = method.clone().view().call().await {
                if let Some((_, previous_tx)) = previous_tx.as_ref() {
                    if let Err(err) = self.archer_api.cancel(previous_tx).await {
                        tracing::error!("archer cancellation failed: {:?}", err);
                    }
                }
                return err;
            }

            // If gas price has increased cancel old and submit new new transaction.

            if let Some((previous_gas_price, previous_tx)) = previous_tx.as_ref() {
                if gas_price > *previous_gas_price {
                    if let Err(err) = self.archer_api.cancel(previous_tx).await {
                        tracing::error!("archer cancellation failed: {:?}", err);
                    }
                } else {
                    tokio::time::sleep(UPDATE_INTERVAL).await;
                    continue;
                }
            }

            // Unwrap because no communication with the node is needed because we specified nonce and gas.
            let (raw_signed_transaction, hash) =
                match method.tx.build().now_or_never().unwrap().unwrap() {
                    Transaction::Request(_) => unreachable!("used local account"),
                    Transaction::Raw { bytes, hash } => (bytes.0, hash),
                };

            tracing::info!(
                "creating archer transaction with hash {:?}, tip to miner {:.3e}, gas price {:.3e}, gas estimate {}",
                hash,
                tx_gas_cost_in_ether_wei.to_f64_lossy(),
                gas_price,
                gas_estimate,
            );

            if let Err(err) = self
                .archer_api
                .submit_transaction(&raw_signed_transaction, deadline)
                .await
            {
                tracing::error!("archer submission failed: {:?}", err);
                tokio::time::sleep(UPDATE_INTERVAL).await;
                continue;
            }

            transactions.push(hash);
            previous_tx = Some((gas_price, raw_signed_transaction));
            tokio::time::sleep(UPDATE_INTERVAL).await;
        }
    }
}

/// From a list of potential hashes find one that was mined.
async fn find_mined_transaction(web3: &Web3, hashes: &[H256]) -> Option<H256> {
    // It would be nice to use the nonce and account address to find the transaction hash but there
    // is no way to do this in ethrpc api so we have to check the candidates one by one.
    let web3 = web3::Web3::new(web3::transports::Batch::new(web3.transport()));
    let futures = hashes
        .iter()
        .map(|&hash| web3.eth().transaction(TransactionId::Hash(hash)))
        .collect::<Vec<_>>();
    if let Err(err) = web3.transport().submit_batch().await {
        tracing::error!("mined transaction batch failed: {:?}", err);
        return None;
    }
    for future in futures {
        match future.now_or_never().unwrap() {
            Err(err) => {
                tracing::error!("mined transaction individual failed: {:?}", err);
            }
            Ok(Some(transaction)) if transaction.block_hash.is_some() => {
                return Some(transaction.hash)
            }
            Ok(_) => (),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::PrivateKey;
    use gas_estimation::GasNowGasStation;
    use hex_literal::hex;
    use reqwest::Client;
    use shared::transport::create_env_test_transport;

    #[tokio::test]
    #[ignore]
    async fn mainnet_find_mined_transaction() {
        let web3 = Web3::new(create_env_test_transport());
        let hashes = &[
            // a non existing transaction
            H256(hex!(
                "b9752d57ea49d8055bf50a1361f066691d7b4f2abd555e71896370d1eccda525"
            )),
            // an existing transaction
            H256(hex!(
                "b9752d57ea49d8055bf50a1361f066691d7b4f2abd555e71896370d1eccda526"
            )),
        ];
        assert_eq!(find_mined_transaction(&web3, hashes).await, Some(hashes[1]));
    }

    // env NODE_URL=... PRIVATE_KEY=... ARCHER_AUTHORIZATION=... cargo test -p solver mainnet_settlement -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet_settlement() {
        shared::tracing::initialize("solver=debug,shared=debug,shared::transport::http=info");

        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        assert_eq!(chain_id, 1);
        let private_key: PrivateKey = std::env::var("PRIVATE_KEY").unwrap().parse().unwrap();
        let account = Account::Offline(private_key, Some(chain_id));
        let contract = crate::get_settlement_contract(&web3).await.unwrap();
        let archer_api = ArcherApi::new(
            std::env::var("ARCHER_AUTHORIZATION").unwrap(),
            Client::new(),
        );
        let gas_price_estimator =
            GasNowGasStation::new(shared::gas_price_estimation::Client(reqwest::Client::new()));
        let gas_price_cap = 100e9;

        let settlement = Settlement::new(Default::default());
        let gas_estimate = crate::settlement_submission::estimate_gas(
            &contract,
            &settlement.clone().into(),
            account.clone(),
        )
        .await
        .unwrap();

        let submitter = ArcherSolutionSubmitter {
            web3: &web3,
            contract: &contract,
            account: &account,
            archer_api: &archer_api,
            gas_price_estimator: &gas_price_estimator,
            gas_price_cap,
        };

        let result = submitter
            .submit(
                Duration::from_secs(0),
                SystemTime::now() + Duration::from_secs(1000),
                settlement,
                gas_estimate,
            )
            .await;
        tracing::info!("finished with result {:?}", result);
    }
}
