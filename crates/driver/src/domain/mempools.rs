use {
    super::{
        competition::{self, solution::settlement},
        eth,
    },
    crate::{
        domain::{
            BlockNo,
            competition::solution::Settlement,
            eth::{TxId, TxStatus},
        },
        infra::{self, Ethereum, observe, solver::Solver},
    },
    anyhow::Context,
    ethrpc::block_stream::into_stream,
    futures::{FutureExt, StreamExt, future::select_ok},
    std::ops::Sub,
    thiserror::Error,
    tracing::Instrument,
};

/// Factor by how much a transaction fee needs to be increased to override a
/// pending transaction at the same nonce.
const GAS_PRICE_BUMP: f64 = 1.125;

/// The gas amount required to cancel a transaction.
const CANCELLATION_GAS_AMOUNT: u64 = 21000;

/// The mempools used to execute settlements.
#[derive(Debug, Clone)]
pub struct Mempools {
    mempools: Vec<infra::Mempool>,
    ethereum: Ethereum,
}

impl Mempools {
    pub fn try_new(mempools: Vec<infra::Mempool>, ethereum: Ethereum) -> Result<Self, NoMempools> {
        if mempools.is_empty() {
            Err(NoMempools)
        } else {
            Ok(Self { mempools, ethereum })
        }
    }

    /// Publish a settlement to the mempools.
    pub async fn execute(
        &self,
        solver: &Solver,
        settlement: &Settlement,
        submission_deadline: BlockNo,
    ) -> Result<eth::TxId, Error> {
        let (submission, _remaining_futures) =
            select_ok(self.mempools.iter().cloned().map(|mempool| {
                async move {
                    let result = self
                        .submit(&mempool, solver, settlement, submission_deadline)
                        .instrument(tracing::info_span!("mempool", kind = mempool.to_string()))
                        .await;
                    observe::mempool_executed(&mempool, settlement, &result);
                    result
                }
                .boxed()
            }))
            .await?;

        Ok(submission.tx_hash)
    }

    /// Defines if the mempools are configured in a way that guarantees that
    /// settled solution will not revert.
    pub fn revert_protection(&self) -> RevertProtection {
        if self.mempools.iter().any(|mempool| {
            matches!(
                mempool.config().kind,
                infra::mempool::Kind::Public {
                    revert_protection: infra::mempool::RevertProtection::Disabled,
                    ..
                }
            )
        }) {
            RevertProtection::Disabled
        } else {
            RevertProtection::Enabled
        }
    }

    async fn submit(
        &self,
        mempool: &infra::mempool::Mempool,
        solver: &Solver,
        settlement: &Settlement,
        submission_deadline: BlockNo,
    ) -> Result<SubmissionSuccess, Error> {
        // Don't submit risky transactions if revert protection is
        // enabled and the settlement may revert in this mempool.
        if settlement.may_revert()
            && matches!(self.revert_protection(), RevertProtection::Enabled)
            && mempool.may_revert()
        {
            return Err(Error::Disabled);
        }

        let tx = settlement.transaction(settlement::Internalization::Enable);

        // Instantiate block stream and skip the current block before we submit the
        // settlement. This way we only run iterations in blocks that can potentially
        // include the settlement.
        let mut block_stream = into_stream(self.ethereum.current_block().clone());
        block_stream.next().await;

        // The tx is simulated before submitting the solution to the competition, but a
        // delay between that and the actual execution can cause the simulation to be
        // invalid which doesn't make sense to submit to the mempool anymore.
        if let Err(err) = self.ethereum.estimate_gas(tx).await {
            if err.is_revert() {
                tracing::info!(
                    ?err,
                    "settlement tx simulation reverted before submitting to the mempool"
                );
                let block = self.ethereum.current_block().borrow().number;
                return Err(Error::SimulationRevert {
                    submitted_at_block: block,
                    reverted_at_block: block,
                });
            } else {
                tracing::warn!(
                    ?err,
                    "couldn't simulate tx before submitting to the mempool"
                );
            }
        }

        let hash = mempool.submit(tx.clone(), settlement.gas, solver).await?;
        let submitted_at_block = self.ethereum.current_block().borrow().number;
        tracing::debug!(?hash, current_block = ?submitted_at_block, "submitted tx to the mempool");

        // Wait for the transaction to be mined, expired or failing.
        let result = async {
            while let Some(block) = block_stream.next().await {
                tracing::debug!(?hash, current_block = ?block.number, "checking if tx is confirmed");
                let receipt = self
                    .ethereum
                    .transaction_status(&hash)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::warn!(?hash, ?err, "failed to get transaction status",);
                        TxStatus::Pending
                    });
                match receipt {
                    TxStatus::Executed { block_number } => return Ok(SubmissionSuccess {
                        tx_hash: hash.clone(),
                        submitted_at_block: submitted_at_block.into(),
                        included_in_block: block_number,
                    }),
                    TxStatus::Reverted { block_number } => {
                        return Err(Error::Revert {
                            tx_id: hash.clone(),
                            submitted_at_block,
                            reverted_at_block: block_number.into(),
                        })
                    }
                    TxStatus::Pending => {
                        let blocks_elapsed = block.number.sub(submitted_at_block);

                        // Check if the current block reached the submission deadline block number
                        if block.number >= submission_deadline {
                            let cancellation_tx_hash = self
                                .cancel(mempool, settlement.gas.price, solver, blocks_elapsed)
                                .await
                                .context("cancellation tx due to deadline failed")?;
                            tracing::info!(
                                settle_tx_hash = ?hash,
                                deadline = submission_deadline,
                                current_block = block.number,
                                ?cancellation_tx_hash,
                                "tx not confirmed in time, cancelling",
                            );
                            return Err(Error::Expired {
                                tx_id: hash.clone(),
                                submitted_at_block,
                                submission_deadline,
                            });
                        }
                        // Check if transaction still simulates
                        if let Err(err) = self.ethereum.estimate_gas(tx).await {
                            if err.is_revert() {
                                let cancellation_tx_hash = self
                                    .cancel(mempool, settlement.gas.price, solver, blocks_elapsed)
                                    .await
                                    .context("cancellation tx due to revert failed")?;
                                tracing::info!(
                                    settle_tx_hash = ?hash,
                                    ?cancellation_tx_hash,
                                    ?err,
                                    "tx started failing in mempool, cancelling"
                                );
                                return Err(Error::SimulationRevert {
                                    submitted_at_block,
                                    reverted_at_block: block.number,
                                });
                            } else {
                                tracing::warn!(?hash, ?err, "couldn't re-simulate tx");
                            }
                        }
                    }
                }
            }
            Err(Error::Other(anyhow::anyhow!(
                "Block stream finished unexpectedly"
            )))
        }
        .await;

        if result.is_err() {
            // Do one last attempt to see if the transaction was confirmed (in case of race
            // conditions or misclassified errors like `OrderFilled` simulation failures).
            if let Ok(TxStatus::Executed { block_number }) =
                self.ethereum.transaction_status(&hash).await
            {
                tracing::info!(
                    ?hash,
                    ?block_number,
                    "Found confirmed transaction, ignoring error"
                );
                return Ok(SubmissionSuccess {
                    tx_hash: hash,
                    included_in_block: block_number,
                    submitted_at_block: submitted_at_block.into(),
                });
            }
        }
        result
    }

    /// Cancel a pending settlement by sending a transaction to self with a
    /// slightly higher gas price than the existing one.
    async fn cancel(
        &self,
        mempool: &infra::mempool::Mempool,
        pending: eth::GasPrice,
        solver: &Solver,
        blocks_elapsed: u64,
    ) -> Result<TxId, Error> {
        let cancellation = eth::Tx {
            from: solver.address(),
            to: solver.address(),
            value: 0.into(),
            input: Default::default(),
            access_list: Default::default(),
        };
        let gas_price_bump_factor = GAS_PRICE_BUMP.powi(blocks_elapsed.max(1) as i32);
        let new_gas_price = pending * gas_price_bump_factor;
        let gas = competition::solution::settlement::Gas {
            estimate: CANCELLATION_GAS_AMOUNT.into(),
            limit: CANCELLATION_GAS_AMOUNT.into(),
            price: new_gas_price,
        };
        tracing::debug!(
            ?blocks_elapsed,
            original_gas_price = ?pending,
            ?new_gas_price,
            bump_factor = ?gas_price_bump_factor,
            "Cancelling transaction with adjusted gas price"
        );

        mempool.submit(cancellation, gas, solver).await
    }
}

pub struct SubmissionSuccess {
    pub tx_hash: eth::TxId,
    /// At which block we started to submit the transaction.
    pub included_in_block: eth::BlockNo,
    /// In which block the transaction actually appeared onchain.
    pub submitted_at_block: eth::BlockNo,
}

#[derive(Debug, Error)]
#[error("no mempools configured, cannot execute settlements")]
pub struct NoMempools;

/// Defines if the mempools are configured in a way that guarantees that
/// /settle'd solution will not revert.
#[derive(Debug, Clone, Copy)]
pub enum RevertProtection {
    Enabled,
    Disabled,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Mined reverted transaction: {tx_id:?}, block number: {reverted_at_block}")]
    Revert {
        tx_id: eth::TxId,
        submitted_at_block: BlockNo,
        reverted_at_block: BlockNo,
    },
    #[error("Simulation started reverting during submission, block number: {reverted_at_block}")]
    SimulationRevert {
        submitted_at_block: BlockNo,
        reverted_at_block: BlockNo,
    },
    #[error(
        "Settlement did not get included in time: submitted at block: {submitted_at_block}, \
         submission deadline: {submission_deadline}, tx: {tx_id:?}"
    )]
    Expired {
        tx_id: eth::TxId,
        submitted_at_block: BlockNo,
        submission_deadline: BlockNo,
    },
    #[error("Strategy disabled for this tx")]
    Disabled,
    #[error("Failed to submit: {0:?}")]
    Other(#[from] anyhow::Error),
}
