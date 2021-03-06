use crate::settlement::{Revertable, Settlement};

use super::{
    super::submitter::{TransactionHandle, TransactionSubmitting},
    AdditionalTip, DisabledReason, Strategy, SubmissionLoopStatus,
};
use anyhow::Result;
use ethcontract::transaction::{Transaction, TransactionBuilder};
use futures::FutureExt;
use shared::{Web3, Web3Transport};

const ALREADY_KNOWN_TRANSACTION: &[&str] = &[
    "Transaction gas price supplied is too low", //openethereum
    "Transaction nonce is too low",              //openethereum
    "already known",                             //infura
    "nonce too low",                             //infura
    "OldNonce",                                  //erigon
    "INTERNAL_ERROR: nonce too low",             //erigon
];

#[derive(Clone)]
pub struct CustomNodesApi {
    nodes: Vec<Web3>,
}

impl CustomNodesApi {
    pub fn new(nodes: Vec<Web3>) -> Self {
        Self { nodes }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for CustomNodesApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
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

    async fn cancel_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        self.submit_transaction(tx).await
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

    fn name(&self) -> Strategy {
        Strategy::CustomNodes
    }
}
