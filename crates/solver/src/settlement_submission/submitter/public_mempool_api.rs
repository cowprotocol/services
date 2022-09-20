use crate::settlement::{Revertable, Settlement};

use super::{
    super::submitter::{TransactionHandle, TransactionSubmitting},
    AdditionalTip, DisabledReason, Strategy, SubmissionLoopStatus,
};
use anyhow::Result;
use ethcontract::transaction::{Transaction, TransactionBuilder};
use futures::FutureExt;
use shared::{Web3, Web3Transport};

#[derive(Clone)]
pub struct PublicMempoolApi {
    nodes: Vec<Web3>,
    high_risk_disabled: bool,
}

impl PublicMempoolApi {
    pub fn new(nodes: Vec<Web3>, high_risk_disabled: bool) -> Self {
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
        let transaction_request = tx.build().now_or_never().unwrap().unwrap();
        let mut futures = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let label = format!("public_mempool_{i}");
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
                        tracing::warn!(?err, %label, "single submission node tx failed");
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
