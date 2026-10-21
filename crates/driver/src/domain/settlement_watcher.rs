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
    ethrpc::block_stream::BlockInfo,
    futures::StreamExt,
    std::ops::ControlFlow,
    tokio_stream::wrappers::WatchStream,
};

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
    /// The settlement tx, to re-simulate it in every block.
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

    async fn on_pending(&self, block: BlockNo) -> ControlFlow<Result<SubmissionSuccess, Error>> {
        let Submitted {
            hash,
            submission_block,
            ..
        } = self.submitted;
        // A node can report a new block before its receipt index catches up,
        // so re-simulating reverts on our own already-applied tx. Our
        // `Settlement` event in any block since submission tells the two
        // apart: `eth_getLogs` sees it even while the receipt lags.
        let (gas, mined) = futures::join!(
            self.ethereum.estimate_gas(self.tx.clone()),
            self.ethereum
                .successful_settlement_block(hash, submission_block.0),
        );

        if let Ok(Some(included_in_block)) = mined {
            // Found on-chain in [submission_block, head]. Scanning the whole
            // range catches it even if the block stream coalesced past the
            // block it mined in.
            return ControlFlow::Break(
                self.event_handler
                    .on_mined(&self.submitted, included_in_block)
                    .await,
            );
        }
        match &gas {
            Err(err) if err.is_revert() => {
                self.event_handler
                    .on_simulation_reverting(&self.submitted, block, err)
                    .await
            }
            Err(err) => {
                tracing::warn!(?hash, ?err, "couldn't re-simulate tx");
                ControlFlow::Continue(())
            }
            Ok(_) => ControlFlow::Continue(()),
        }
    }
}
