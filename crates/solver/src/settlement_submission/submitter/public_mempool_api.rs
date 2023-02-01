use crate::settlement::{Revertable, Settlement};

use super::{
    super::submitter::{TransactionHandle, TransactionSubmitting},
    AdditionalTip, DisabledReason, Strategy, SubmissionLoopStatus,
};
use anyhow::{ensure, Context, Result};
use ethcontract::transaction::{Transaction, TransactionBuilder};
use futures::FutureExt;
use reqwest::Url;
use shared::{
    ethrpc::{self, Web3, Web3Transport},
    http_client::HttpClientFactory,
};
use std::ops::Deref;

#[derive(Clone)]
pub struct PublicMempoolApi {
    nodes: Vec<SubmissionNode>,
    high_risk_disabled: bool,
}

impl PublicMempoolApi {
    pub fn new(nodes: Vec<SubmissionNode>, high_risk_disabled: bool) -> Self {
        Self {
            nodes,
            high_risk_disabled,
        }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for PublicMempoolApi {
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle> {
        tracing::debug!("public mempool submit transaction entered");
        let transaction_request = tx.build().await.unwrap();
        if let Transaction::Raw { hash, .. } = &transaction_request {
            tracing::debug!(?hash, "creating transaction");
        }
        let mut futures = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let label = format!(
                    "public_mempool_{i}_{}",
                    match node {
                        SubmissionNode::Broadcast(_) => "broadcast",
                        SubmissionNode::Notification(_) => "notification",
                    }
                );
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

        let mut errors = vec![];
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
                    let err = err.to_string();

                    // Collect all errors to allow caller to react to all of them.
                    errors.push(format!("{label} failed to submit: {err}"));

                    // Due to the highly decentralized nature of tx submission an error suggesting
                    // that a tx was already mined or is underpriced is benign and should not be
                    // reported to avoid triggering alerts unnecessarily.
                    let is_benign_error = super::TX_ALREADY_MINED
                        .iter()
                        .chain(super::TX_ALREADY_KNOWN)
                        .any(|msg| err.contains(msg));
                    super::track_submission_success(&label, is_benign_error);

                    if !is_benign_error {
                        tracing::warn!(%err, %label, "single submission node tx failed");
                    }

                    if rest.is_empty() {
                        return Err(anyhow::anyhow!(errors.join("\n"))
                            .context("all submission nodes failed"));
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

    fn submission_status(&self, settlement: &Settlement, _: &str) -> SubmissionLoopStatus {
        // disable strategy if there is a slightest possibility for a transaction to be reverted (check done only for mainnet)
        if self.high_risk_disabled && settlement.revertable() == Revertable::HighRisk {
            return SubmissionLoopStatus::Disabled(DisabledReason::MevExtractable);
        }

        SubmissionLoopStatus::Enabled(AdditionalTip::Off)
    }

    fn name(&self) -> Strategy {
        Strategy::PublicMempool
    }
}

#[derive(Debug, Clone)]
pub enum SubmissionNode {
    /// Transactions that are sent to this nodes are expected to be broadcast to the mempool and
    /// eventually be included in a block.
    Broadcast(Web3),
    /// A notification node is an endpoint that is not expected to submit transactions to the
    /// mempool once a transaction has been received. Its purpose is notifying the node owner that a
    /// transaction has been submitted.
    /// In general, there are lower expectations on the availability of this node variant.
    Notification(Web3),
}

impl Deref for SubmissionNode {
    type Target = Web3;

    fn deref(&self) -> &Self::Target {
        match self {
            SubmissionNode::Broadcast(web3) => web3,
            SubmissionNode::Notification(web3) => web3,
        }
    }
}

impl SubmissionNode {
    pub async fn validated_broadcast_node(
        ethrpc_configs: &ethrpc::Arguments,
        http_factory: &HttpClientFactory,
        url: Url,
        name: impl ToString,
        expected_network_id: &String,
    ) -> Result<Self> {
        let node = ethrpc::web3(ethrpc_configs, http_factory, &url, name);
        validate_submission_node(&node, expected_network_id)
            .await
            .with_context(|| format!("Validation error for broadcast node {url}"))?;
        Ok(Self::Broadcast(node))
    }

    pub async fn from_notification_url(
        ethrpc_configs: &ethrpc::Arguments,
        http_factory: &HttpClientFactory,
        url: Url,
        name: impl ToString,
        expected_network_id: &String,
    ) -> Self {
        let node = ethrpc::web3(ethrpc_configs, http_factory, &url, name);
        if let Err(err) = validate_submission_node(&node, expected_network_id).await {
            tracing::error!("Error validating submission notification node {url}: {err}");
        }
        Self::Notification(node)
    }

    pub fn can_broadcast(&self) -> bool {
        match self {
            SubmissionNode::Broadcast(_) => true,
            SubmissionNode::Notification(_) => false,
        }
    }
}

pub async fn validate_submission_node(node: &Web3, expected_network_id: &String) -> Result<()> {
    let node_network_id = node
        .net()
        .version()
        .await
        .context("Unable to retrieve network id on startup")?;
    ensure!(
        &node_network_id == expected_network_id,
        "Network id doesn't match expected network id"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::NoopInteraction;

    #[test]
    fn submission_status_configuration() {
        let high_risk_settlement = {
            let mut settlement = Settlement::new(Default::default());
            settlement.encoder.append_to_execution_plan(NoopInteraction);
            assert_eq!(settlement.revertable(), Revertable::HighRisk);
            settlement
        };

        let submitter = PublicMempoolApi::new(vec![], false);
        assert_eq!(
            submitter.submission_status(&high_risk_settlement, ""),
            SubmissionLoopStatus::Enabled(AdditionalTip::Off),
        );

        let submitter = PublicMempoolApi::new(vec![], true);
        assert_eq!(
            submitter.submission_status(&high_risk_settlement, ""),
            SubmissionLoopStatus::Disabled(DisabledReason::MevExtractable),
        );
    }
}
