use {
    super::{
        super::submitter::{TransactionHandle, TransactionSubmitting},
        DisabledReason,
        Strategy,
        SubmissionLoopStatus,
    },
    crate::{settlement::Revertable, settlement_submission::SubmitterSettlement},
    anyhow::{ensure, Context, Result},
    ethcontract::{
        transaction::{Transaction, TransactionBuilder},
        H256,
    },
    futures::FutureExt,
    shared::ethrpc::{Web3, Web3Transport},
};

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
        let (notification_nodes, other_nodes) = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                (
                    node,
                    format!(
                        "public_mempool_{i}_{}",
                        match node.kind {
                            SubmissionNodeKind::Broadcast => "broadcast",
                            SubmissionNodeKind::Notification => "notification",
                        }
                    ),
                )
            })
            .partition::<Vec<_>, _>(|(node, _)| node.kind == SubmissionNodeKind::Notification);

        // We are not interested in the response from a notification node.
        for (node, label) in notification_nodes {
            let transaction_request = transaction_request.clone();
            let node = node.clone();
            tokio::spawn(async move {
                match submit_transaction(transaction_request, &node, &label).await {
                    Ok(tx_hash) => {
                        tracing::debug!(%label, "notification node reports transaction creation with hash: {:?}", tx_hash);
                    }
                    Err(err) => {
                        tracing::debug!(%label, "notification node reports failed transaction creation: {}", err.to_string());
                    }
                };
            });
        }

        let mut futures = other_nodes
            .into_iter()
            .map(|(node, label)| {
                let transaction_request = transaction_request.clone();
                (async move {
                    (
                        submit_transaction(transaction_request, node, &label).await,
                        label,
                    )
                })
                .boxed()
            })
            .collect::<Vec<_>>();
        let mut errors = vec![];
        loop {
            let ((result, label), _, rest) = futures::future::select_all(futures).await;
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

    fn submission_status(&self, settlement: &SubmitterSettlement, _: &str) -> SubmissionLoopStatus {
        // disable strategy if there is a slightest possibility for a transaction to be
        // reverted (check done only for mainnet)
        if self.high_risk_disabled && settlement.inner.revertable() == Revertable::HighRisk {
            return SubmissionLoopStatus::Disabled(DisabledReason::MevExtractable);
        }

        SubmissionLoopStatus::Enabled
    }

    fn name(&self) -> Strategy {
        Strategy::PublicMempool
    }
}

async fn submit_transaction(
    transaction_request: Transaction,
    node: &SubmissionNode,
    label: &String,
) -> Result<H256, web3::Error> {
    tracing::debug!(%label, "sending transaction...");
    match transaction_request {
        Transaction::Request(tx) => node.web3.eth().send_transaction(tx).await,
        Transaction::Raw { bytes, .. } => {
            node.web3.eth().send_raw_transaction(bytes.0.into()).await
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionNodeKind {
    /// Transactions that are sent to this nodes are expected to be broadcast to
    /// the mempool and eventually be included in a block.
    Broadcast,
    /// A notification node is an endpoint that is not expected to submit
    /// transactions to the mempool once a transaction has been received.
    /// Its purpose is notifying the node owner that a transaction has been
    /// submitted. In general, there are lower expectations on the
    /// availability of this node variant.
    Notification,
}

#[derive(Debug, Clone)]
pub struct SubmissionNode {
    kind: SubmissionNodeKind,
    web3: Web3,
}

impl SubmissionNode {
    pub fn new(kind: SubmissionNodeKind, web3: Web3) -> Self {
        Self { kind, web3 }
    }

    pub fn can_broadcast(&self) -> bool {
        match self.kind {
            SubmissionNodeKind::Broadcast => true,
            SubmissionNodeKind::Notification => false,
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
    use {
        super::*,
        crate::settlement::{NoopInteraction, Settlement},
        std::sync::Arc,
    };

    #[test]
    fn submission_status_configuration() {
        let high_risk_settlement = {
            let mut settlement = Settlement::default();
            settlement
                .encoder
                .append_to_execution_plan(Arc::new(NoopInteraction));
            assert_eq!(settlement.revertable(), Revertable::HighRisk);
            SubmitterSettlement::for_settlement(settlement)
        };

        let submitter = PublicMempoolApi::new(vec![], false);
        assert_eq!(
            submitter.submission_status(&high_risk_settlement, ""),
            SubmissionLoopStatus::Enabled,
        );

        let submitter = PublicMempoolApi::new(vec![], true);
        assert_eq!(
            submitter.submission_status(&high_risk_settlement, ""),
            SubmissionLoopStatus::Disabled(DisabledReason::MevExtractable),
        );
    }
}
