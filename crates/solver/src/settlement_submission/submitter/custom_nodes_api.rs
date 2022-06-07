use crate::{
    pending_transactions::Fee,
    settlement::{Revertable, Settlement},
};

use super::{
    super::submitter::{TransactionHandle, TransactionSubmitting},
    AdditionalTip, CancelHandle, DisabledReason, SubmissionLoopStatus,
};
use anyhow::{Context, Result};
use ethcontract::{
    dyns::DynTransport,
    transaction::{Transaction, TransactionBuilder},
    H160, U256,
};
use futures::FutureExt;
use gas_estimation::GasPrice1559;
use shared::Web3;

const ALREADY_KNOWN_TRANSACTION: &[&str] = &[
    "Transaction gas price supplied is too low", //openethereum
    "Transaction nonce is too low",              //openethereum
    "already known",                             //infura
    "nonce too low",                             //infura
    "OldNonce",                                  //erigon
    "INTERNAL_ERROR: nonce too low",             //erigon
];

#[derive(Copy, Clone, Debug, clap::ArgEnum)]
pub enum PendingTransactionConfig {
    /// Attempt to fetch pending transactions using txpool_content call. This can cause problems
    /// when nodes return a large amount of data or are slow to respond.
    TxPool,
    /// Do not attempt to fetch pending transactions.
    Ignore,
}

#[derive(Clone)]
pub struct CustomNodesApi {
    nodes: Vec<Web3>,
    pending_transaction_config: PendingTransactionConfig,
}

impl CustomNodesApi {
    pub fn new(nodes: Vec<Web3>, pending_transaction_config: PendingTransactionConfig) -> Self {
        Self {
            nodes,
            pending_transaction_config,
        }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for CustomNodesApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<DynTransport>,
    ) -> Result<TransactionHandle> {
        tracing::debug!("Custom nodes submit transaction entered");
        let transaction_request = tx.build().now_or_never().unwrap().unwrap();
        let mut futures = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let label = format!("custom_nodes_{i}");
                let transaction_request = transaction_request.clone();
                async move {
                    tracing::debug!(%label, "sending transaction...");
                    let result = match transaction_request {
                        Transaction::Request(tx) => node.eth().send_transaction(tx).await,
                        Transaction::Raw { bytes, .. } => {
                            node.eth().send_raw_transaction(bytes.0.into()).await
                        }
                    };

                    (label, result)
                }
                .boxed()
            })
            .collect::<Vec<_>>();

        loop {
            let ((label, result), _, rest) = futures::future::select_all(futures).await;
            match result {
                Ok(tx_hash) => {
                    super::track_submission_success(&label, true);
                    tracing::debug!(%label, "created transaction with hash: {:?}", tx_hash);
                    return Ok(TransactionHandle {
                        tx_hash,
                        handle: tx_hash,
                    });
                }
                Err(err) => {
                    if matches!(
                        &err,
                        web3::Error::Rpc(rpc_error)
                            if ALREADY_KNOWN_TRANSACTION
                                .iter()
                                .any(|message| rpc_error.message.starts_with(message))
                    ) {
                        tracing::debug!(%label, ?transaction_request, "transaction already known");
                        // error is not real error if transaction pool responded that received transaction is
                        // already in the pool, meaning that the transaction was created successfully and we can
                        // continue searching our futures for a successful node RPC response without incrementing
                        // any error metrics...
                    } else {
                        tracing::warn!(?err, %label, "single custom node tx failed");
                        super::track_submission_success(&label, false);
                    }

                    if rest.is_empty() {
                        return Err(anyhow::Error::from(err).context("all custom nodes tx failed"));
                    }
                    futures = rest;
                }
            }
        }
    }

    async fn cancel_transaction(&self, id: &CancelHandle) -> Result<TransactionHandle> {
        self.submit_transaction(id.noop_transaction.clone()).await
    }

    async fn recover_pending_transaction(
        &self,
        web3: &Web3,
        address: &H160,
        nonce: U256,
    ) -> Result<Option<GasPrice1559>> {
        match self.pending_transaction_config {
            PendingTransactionConfig::Ignore => return Ok(None),
            PendingTransactionConfig::TxPool => (),
        }
        let transactions = crate::pending_transactions::pending_transactions(web3.transport())
            .await
            .context("pending_transactions failed")?;
        let transaction = match transactions
            .iter()
            .find(|transaction| transaction.from == *address && transaction.nonce == nonce)
        {
            Some(transaction) => transaction,
            None => return Ok(None),
        };
        match transaction.fee {
            Fee::Legacy { gas_price } => Ok(Some(GasPrice1559 {
                base_fee_per_gas: 0.0,
                max_fee_per_gas: gas_price.to_f64_lossy(),
                max_priority_fee_per_gas: gas_price.to_f64_lossy(),
            })),
            Fee::Eip1559 {
                max_priority_fee_per_gas,
                max_fee_per_gas,
            } => Ok(Some(GasPrice1559 {
                max_fee_per_gas: max_fee_per_gas.to_f64_lossy(),
                max_priority_fee_per_gas: max_priority_fee_per_gas.to_f64_lossy(),
                base_fee_per_gas: crate::pending_transactions::base_fee_per_gas(web3.transport())
                    .await?
                    .to_f64_lossy(),
            })),
        }
    }

    fn submission_status(&self, settlement: &Settlement, network_id: &str) -> SubmissionLoopStatus {
        // disable strategy if there is a slightest possibility for a transaction to be reverted (check done only for mainnet)
        if shared::gas_price_estimation::is_mainnet(network_id) {
            if let Revertable::HighRisk = settlement.revertable() {
                return SubmissionLoopStatus::Disabled(DisabledReason::MevExtractable);
            }
        }

        SubmissionLoopStatus::Enabled(AdditionalTip::Off)
    }

    fn name(&self) -> &'static str {
        "CustomNodes"
    }
}
