use {
    super::{competition::solution::settlement, eth},
    crate::{
        domain::{
            BlockNo,
            competition::solution::Settlement,
            eth::{TxId, TxStatus},
        },
        infra::{self, Ethereum, observe, solver::Solver},
    },
    alloy::{consensus::Transaction, providers::ext::TxPoolApi},
    anyhow::Context,
    ethrpc::{alloy::conversions::IntoAlloy, block_stream::into_stream},
    futures::{FutureExt, StreamExt, future::select_ok},
    std::ops::Sub,
    thiserror::Error,
    tracing::Instrument,
};

/// Factor by how much a transaction fee needs to be increased to override a
/// pending transaction at the same nonce. The correct factor is actually
/// 1.25 but to avoid rounding issues on chains with very low gas prices
/// we increase slightly more.
const GAS_PRICE_BUMP: f64 = 1.3;

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

        let current_block = self.ethereum.current_block().borrow().number;
        // The tx is simulated before submitting the solution to the competition, but a
        // delay between that and the actual execution can cause the simulation to be
        // invalid which doesn't make sense to submit to the mempool anymore.
        if let Err(err) = self.ethereum.estimate_gas(tx).await {
            if err.is_revert() {
                tracing::info!(
                    ?err,
                    "settlement tx simulation reverted before submitting to the mempool"
                );
                return Err(Error::SimulationRevert {
                    submitted_at_block: current_block,
                    reverted_at_block: current_block,
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

        let replacement_gas_price = self
            .minimum_replacement_gas_price_based_on_mempool(solver, nonce)
            .await;
        let current_gas_price = self
            .ethereum
            .gas_price(None)
            .await
            .unwrap_or(settlement.gas.price);

        let minimum_gas_price = if let Ok(replacement_gas_price) = replacement_gas_price {
            if replacement_gas_price.max() > current_gas_price.max() {
                replacement_gas_price
            } else {
                current_gas_price
            }
        } else {
            current_gas_price
        };

        // bump the gas price such that it would still be good if the gas price
        // increases the maximum value for every block until the deadline
        let blocks_until_deadline = submission_deadline.saturating_sub(current_block);
        let final_gas_price = minimum_gas_price * GAS_PRICE_BUMP.powi(blocks_until_deadline as i32);

        let hash = match mempool
            .submit(
                tx.clone(),
                final_gas_price,
                settlement.gas.limit,
                solver,
                nonce,
            )
            .await
        {
            Ok(hash) => hash,
            Err(err) => {
                let pending_tx = self.find_pending_tx_in_mempool(solver, nonce).await;
                tracing::warn!(
                    ?nonce,
                    ?solver,
                    ?settlement.gas,
                    ?err,
                    ?pending_tx,
                    "failed to submit settlement tx"
                );
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
        original_tx_gas_price: eth::GasPrice,
        solver: &Solver,
        blocks_elapsed: u64,
        nonce: eth::U256,
    ) -> Result<TxId, Error> {
        let gas_price = if let Ok(replacement_gas_price) = self
            .minimum_replacement_gas_price_based_on_mempool(solver, nonce)
            .await
        {
            // we trust our RPC's mempool the most as this is the actual
            // source of truth for the replacement gas price
            replacement_gas_price
        } else {
            // if we were not able to find an existing tx in the RPC's
            // mempool we have to assume that we just have to beat the
            // original tx's gas price
            original_tx_gas_price * GAS_PRICE_BUMP
        };

        let cancellation = eth::Tx {
            from: solver.address(),
            to: solver.address(),
            value: 0.into(),
            input: Default::default(),
            access_list: Default::default(),
        };

        // TODO move logging into `submit()`
        tracing::debug!(
            ?blocks_elapsed,
            ?gas_price,
            ?nonce,
            "Cancelling transaction with adjusted gas price"
        );

        mempool
            .submit(
                cancellation,
                gas_price,
                CANCELLATION_GAS_AMOUNT.into(),
                solver,
                nonce,
            )
            .await
    }

    /// Queries the connected RPC for a pending transaction
    /// for the given solver and nonce.
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

    /// Tries to determine the minimum price to replace an existing
    /// transaction in the mempool.
    async fn minimum_replacement_gas_price_based_on_mempool(
        &self,
        solver: &Solver,
        nonce: eth::U256,
    ) -> anyhow::Result<eth::GasPrice> {
        let pending_tx = self.find_pending_tx_in_mempool(solver, nonce).await?;
        let pending_tx_gas_price = eth::GasPrice {
            max: eth::U256::from(pending_tx.max_fee_per_gas()).into(),
            tip: eth::U256::from(
                pending_tx
                    .max_priority_fee_per_gas()
                    .context("pending tx is not EIP 1559")?,
            )
            .into(),
            base: eth::U256::from(pending_tx.max_fee_per_gas()).into(),
        };
        // in order to replace a tx we need to increase the price
        Ok(pending_tx_gas_price * GAS_PRICE_BUMP)
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
