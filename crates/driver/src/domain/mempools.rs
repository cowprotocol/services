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
        util::conv::u256::U256Ext,
    },
    alloy::{consensus::Transaction, providers::ext::TxPoolApi},
    anyhow::Context,
    ethcontract::U256,
    ethrpc::{alloy::conversions::IntoAlloy, block_stream::into_stream},
    futures::{FutureExt, StreamExt, future::select_ok},
    std::ops::Sub,
    thiserror::Error,
    tracing::Instrument,
};

/// Factor by how much a transaction fee needs to be increased to override a
/// pending transaction at the same nonce.
const GAS_PRICE_BUMP: f64 = 1.13;

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

        // Fetch the nonce to avoid race conditions between concurrent
        // transactions (e.g., settlement tx and cancellation tx) from the same
        // solver address.
        let nonce = mempool.get_nonce(solver.address()).await?;

        let gas = settlement::Gas {
            price: self
                .replacement_gas_price(solver, nonce, settlement.gas.price)
                .await,
            ..settlement.gas
        };
        let hash = match mempool.submit(tx.clone(), gas, solver, nonce).await {
            Ok(hash) => hash,
            Err(err) => {
                let pending_tx = self.find_pending_tx_in_mempool(solver, nonce).await;
                tracing::warn!(?tx, ?nonce, ?solver, ?settlement.gas, ?err, ?pending_tx, "failed to submit settlement tx");
                return Err(err);
            }
        };
        let submitted_at_block = self.ethereum.current_block().borrow().number;
        tracing::debug!(
            ?hash,
            current_block = ?submitted_at_block,
            max_fee_per_gas = ?settlement.gas.price.max(),
            priority_fee_per_gas = ?settlement.gas.price.tip(),
            ?nonce,
            "submitted tx to the mempool"
        );

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
                                .cancel(mempool, settlement.gas.price, solver, blocks_elapsed, nonce)
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
                                    .cancel(mempool, settlement.gas.price, solver, blocks_elapsed, nonce)
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
        nonce: eth::U256,
    ) -> Result<TxId, Error> {
        let new_gas_price = self.replacement_gas_price(solver, nonce, pending).await;

        let cancellation = eth::Tx {
            from: solver.address(),
            to: solver.address(),
            value: 0.into(),
            input: Default::default(),
            access_list: Default::default(),
        };
        let gas = competition::solution::settlement::Gas {
            estimate: CANCELLATION_GAS_AMOUNT.into(),
            limit: CANCELLATION_GAS_AMOUNT.into(),
            price: new_gas_price,
        };
        tracing::debug!(
            ?blocks_elapsed,
            ?new_gas_price,
            ?nonce,
            "Cancelling transaction with adjusted gas price"
        );

        mempool.submit(cancellation, gas, solver, nonce).await
    }

    async fn find_pending_tx_in_mempool(
        &self,
        solver: &Solver,
        nonce: eth::U256,
    ) -> anyhow::Result<alloy::rpc::types::Transaction> {
        let tx_pool_content = self
            .ethereum
            .web3()
            .alloy
            .txpool_content_from(solver.address().0.into_alloy())
            .await
            .context("failed to query pending transactions")?;

        // find the one with the specified nonce
        let pending_tx = tx_pool_content
            .pending
            .into_iter()
            .chain(tx_pool_content.queued)
            .find(|(_signer, tx)| tx.nonce() == nonce.as_u64())
            .context("no pending transaction with target nonce ({nonce})")?
            .1;
        Ok(pending_tx)
    }

    async fn replacement_gas_price_based_on_mempool(
        &self,
        solver: &Solver,
        nonce: eth::U256,
    ) -> anyhow::Result<eth::GasPrice> {
        let tx = self.find_pending_tx_in_mempool(solver, nonce).await?;
        let replacement_gas_price = eth::GasPrice::new(
            U256::from(tx.max_fee_per_gas())
                .checked_mul_f64(GAS_PRICE_BUMP)
                .context("gas price bump overflowed the max_fee_per_gas")?
                .into(),
            eth::U256::from(
                tx.max_priority_fee_per_gas()
                    .context("pending tx is not EIP 1559")?,
            )
            .checked_mul_f64(GAS_PRICE_BUMP)
            .context("gas price bump overflowed the max_priority_fee_per_gas")?
            .into(),
            U256::from(tx.max_fee_per_gas()).into(),
        );
        Ok(replacement_gas_price)
    }

    async fn replacement_gas_price(
        &self,
        solver: &Solver,
        nonce: eth::U256,
        current_gas_price: eth::GasPrice,
    ) -> eth::GasPrice {
        match self
            .replacement_gas_price_based_on_mempool(solver, nonce)
            .await
        {
            Ok(gas) => {
                tracing::debug!(?gas, "computed replacement gas based on mempool");
                gas
            }
            Err(err) => {
                let bumped = current_gas_price * GAS_PRICE_BUMP;
                tracing::warn!(
                    current = ?current_gas_price,
                    replacement_gas = ?bumped,
                    ?err,
                    "failed to compute gas price based on mempool - fall back to current gas price"
                );
                bumped
            }
        }
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
