use {
    crate::{
        domain::{
            blockchain::TxStatus,
            mempools::{Error, SubmissionSuccess},
        },
        infra::{Ethereum, blockchain},
    },
    anyhow::anyhow,
    eth_domain_types::{self as eth, BlockNo, TxId},
    ethrpc::block_stream::{BlockInfo, into_stream},
    futures::StreamExt,
    std::ops::ControlFlow,
    tokio_stream::wrappers::WatchStream,
};

/// The blocks after the current one: the ones that can include a tx sent now.
pub(crate) async fn next_blocks(ethereum: &Ethereum) -> WatchStream<BlockInfo> {
    let mut blocks = into_stream(ethereum.current_block().clone());
    blocks.next().await;
    blocks
}

/// A settlement tx that was sent and waits to get mined.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Submitted {
    pub hash: TxId,
    pub submission_block: BlockNo,
    pub submission_deadline: BlockNo,
}

impl Submitted {
    pub fn mined(&self, included_in_block: BlockNo) -> SubmissionSuccess {
        SubmissionSuccess {
            tx_hash: self.hash,
            submitted_at_block: self.submission_block,
            included_in_block,
        }
    }

    pub fn expired(&self) -> Error {
        Error::Expired {
            tx_id: self.hash,
            submitted_at_block: self.submission_block,
            submission_deadline: self.submission_deadline,
        }
    }
}

/// What to do when a settlement is still pending in a new block. Executed
/// and reverted receipts always end the watch.
#[async_trait::async_trait]
pub(crate) trait SettlementEventHandler: Send + Sync {
    /// Whether a sent tx can still be cancelled. If not, there is nothing to
    /// do for a pending settlement, so the watcher does not re-simulate it.
    fn supports_cancellation(&self) -> bool;

    /// The deadline block came and the settlement is not mined.
    async fn on_submission_expired(&self, submitted: &Submitted) -> Error {
        submitted.expired()
    }

    /// `eth_getLogs` found the settlement while its receipt still lags.
    async fn on_mined(
        &self,
        submitted: &Submitted,
        included_in_block: BlockNo,
    ) -> Result<SubmissionSuccess, Error> {
        Ok(submitted.mined(included_in_block))
    }

    /// The settlement reverts when simulated on top of `block`.
    async fn on_simulation_reverting(
        &self,
        _submitted: &Submitted,
        _block: BlockNo,
        _err: &blockchain::Error,
    ) -> ControlFlow<Result<SubmissionSuccess, Error>> {
        ControlFlow::Continue(())
    }
}

/// Watches the blocks after a settlement was sent until it is mined, reverts
/// or the submission deadline passes.
pub(crate) struct SettlementWatcher<'a, H> {
    pub ethereum: &'a Ethereum,
    /// The settlement tx, re-simulated in every block if it can be cancelled.
    pub tx: &'a eth::Tx,
    pub submitted: Submitted,
    pub event_handler: H,
}

impl<H: SettlementEventHandler> SettlementWatcher<'_, H> {
    pub async fn watch(
        &self,
        mut blocks: WatchStream<BlockInfo>,
    ) -> Result<SubmissionSuccess, Error> {
        let Submitted {
            hash,
            submission_block,
            submission_deadline,
        } = self.submitted;
        let result = async {
            while let Some(block) = blocks.next().await {
                let block = BlockNo(block.number);
                tracing::debug!(?hash, current_block = ?block, "checking if tx is confirmed");
                let status = self
                    .ethereum
                    .transaction_status(&hash)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::warn!(?hash, ?err, "failed to get transaction status");
                        TxStatus::Pending
                    });
                match status {
                    TxStatus::Executed { block_number } => {
                        return Ok(self.submitted.mined(block_number));
                    }
                    TxStatus::Reverted { block_number } => {
                        return Err(Error::Revert {
                            tx_id: hash,
                            submitted_at_block: submission_block,
                            reverted_at_block: block_number,
                        });
                    }
                    TxStatus::Pending if block >= submission_deadline => {
                        if let Some(included_in_block) = self.mined_with_lagging_receipt().await {
                            return self
                                .event_handler
                                .on_mined(&self.submitted, included_in_block)
                                .await;
                        }
                        tracing::debug!(
                            submission_deadline = self.submitted.submission_deadline.0,
                            settle_tx_hash = ?self.submitted.hash,
                            "exceeded submission deadline"
                        );
                        return Err(self
                            .event_handler
                            .on_submission_expired(&self.submitted)
                            .await);
                    }
                    TxStatus::Pending if !self.event_handler.supports_cancellation() => {}
                    TxStatus::Pending => {
                        if let ControlFlow::Break(result) = self.on_pending(block).await {
                            return result;
                        }
                    }
                }
            }
            Err(Error::Other(anyhow!("Block stream finished unexpectedly")))
        }
        .await;

        if result.is_err() {
            // One last check in case the tx landed after the loop exited, e.g.
            // the receipt finally caught up to a block we had already
            // processed.
            if let Ok(TxStatus::Executed { block_number }) =
                self.ethereum.transaction_status(&hash).await
            {
                tracing::info!(
                    ?hash,
                    ?block_number,
                    "Found confirmed transaction, ignoring error"
                );
                return Ok(self.submitted.mined(block_number));
            }
        }
        result
    }

    /// The block of our `Settlement` event, which shows up before a lagging
    /// receipt does.
    async fn mined_with_lagging_receipt(&self) -> Option<BlockNo> {
        let Submitted {
            hash,
            submission_block,
            ..
        } = self.submitted;
        self.ethereum
            .successful_settlement_block(hash, submission_block.0)
            .await
            .ok()
            .flatten()
    }

    async fn on_pending(&self, block: BlockNo) -> ControlFlow<Result<SubmissionSuccess, Error>> {
        // A node can report a new block before its receipt index catches up,
        // so re-simulating reverts on our own already-applied tx. Our
        // `Settlement` event tells the two apart.
        let (gas, mined) = futures::join!(
            self.ethereum.estimate_gas(self.tx.clone()),
            self.mined_with_lagging_receipt(),
        );

        if let Some(included_in_block) = mined {
            return ControlFlow::Break(
                self.event_handler
                    .on_mined(&self.submitted, included_in_block)
                    .await,
            );
        }
        match &gas {
            Err(err) if err.is_revert() => {
                tracing::debug!(hash = ?self.submitted.hash, ?err, "simulation started reverting");
                self.event_handler
                    .on_simulation_reverting(&self.submitted, block, err)
                    .await
            }
            Err(err) => {
                tracing::warn!(hash = ?self.submitted.hash, ?err, "couldn't re-simulate tx");
                ControlFlow::Continue(())
            }
            Ok(_) => ControlFlow::Continue(()),
        }
    }
}
