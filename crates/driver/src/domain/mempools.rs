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
    alloy::{consensus::Transaction, eips::eip1559::Eip1559Estimation},
    anyhow::Context,
    ethrpc::block_stream::into_stream,
    futures::{FutureExt, StreamExt, future::select_ok},
    thiserror::Error,
    tracing::Instrument,
};

/// Factor by how much a transaction fee needs to be increased to override a
/// pending transaction at the same nonce. The correct factor is actually
/// 12.5% but to avoid rounding issues on chains with very low gas prices
/// we increase slightly more.
const GAS_PRICE_BUMP_PCT: u64 = 13;

/// The gas amount required to cancel a transaction.
const CANCELLATION_GAS_AMOUNT: u64 = 21000;

/// Context for EIP-7702 delegated submission. When present, a dedicated
/// submission EOA signs and pays for the tx while routing it through the
/// solver's delegated code.
#[derive(Debug, Clone)]
pub struct DelegatedSubmission {
    /// The address that signs the transaction and whose nonce is used.
    pub signer: eth::Address,
    /// The solver EOA address. In EIP-7702 mode tx.to is set to this address
    /// (which delegates to a forwarder contract), instead of the settlement
    /// contract.
    pub solver_eoa: eth::Address,
}

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

    /// Publish a settlement to the mempools. When `delegated` is `Some`, the
    /// settlement is submitted via an EIP-7702 submission account instead of
    /// the solver directly.
    pub async fn execute(
        &self,
        solver: &Solver,
        settlement: &Settlement,
        submission_deadline: BlockNo,
        delegated: Option<&DelegatedSubmission>,
    ) -> Result<eth::TxId, Error> {
        let (submission, _remaining_futures) = select_ok(self.mempools.iter().map(|mempool| {
            async move {
                let result = self
                    .submit(mempool, solver, settlement, submission_deadline, delegated)
                    .instrument(tracing::info_span!("mempool", kind = mempool.to_string()))
                    .await;
                observe::mempool_executed(mempool, settlement, &result);
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
        match self
            .mempools
            .iter()
            .all(|mempool| mempool.reverts_can_get_mined())
        {
            true => RevertProtection::Disabled,
            false => RevertProtection::Enabled,
        }
    }

    async fn submit(
        &self,
        mempool: &infra::mempool::Mempool,
        solver: &Solver,
        settlement: &Settlement,
        submission_deadline: BlockNo,
        delegated: Option<&DelegatedSubmission>,
    ) -> Result<SubmissionSuccess, Error> {
        // Don't submit risky transactions if revert protection is
        // enabled and the settlement may revert in this mempool.
        if settlement.may_revert()
            && matches!(self.revert_protection(), RevertProtection::Enabled)
            && mempool.reverts_can_get_mined()
        {
            return Err(Error::Disabled);
        }

        let tx = settlement.transaction(settlement::Internalization::Enable);

        // In EIP-7702 mode, reroute the tx to the solver EOA (which delegates
        // to a forwarder contract). The calldata stays identical.
        let tx = match delegated {
            Some(ctx) => {
                let mut tx = tx.clone();
                tx.to = ctx.solver_eoa;
                tx
            }
            None => tx.clone(),
        };

        // The address that signs and pays for gas.
        let signer = delegated
            .map(|ctx| ctx.signer)
            .unwrap_or_else(|| solver.address());

        // Instantiate block stream and skip the current block before we submit the
        // settlement. This way we only run iterations in blocks that can potentially
        // include the settlement.
        let mut block_stream = into_stream(self.ethereum.current_block().clone());
        block_stream.next().await;

        let current_block = self.ethereum.current_block().borrow().number;
        // The tx is simulated before submitting the solution to the competition, but a
        // delay between that and the actual execution can cause the simulation to be
        // invalid which doesn't make sense to submit to the mempool anymore.
        // NOTE: in 7702 mode we simulate the original tx (via solver address) since
        // estimate_gas uses eth_call which doesn't need 7702 delegation to work.
        let sim_tx = settlement.transaction(settlement::Internalization::Enable);
        if mempool.reverts_can_get_mined() {
            if let Err(err) = self.ethereum.estimate_gas(sim_tx.clone()).await {
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
        } else {
            tracing::trace!("skipping tx simulation because mempool does not mine reverting txs");
        }

        // Fetch the nonce for the signing account (not the solver in 7702 mode).
        let nonce = mempool.get_nonce(signer).await?;

        // estimate the gas price such that the tx should still be included
        // even if the gas price increases the maximum amount until the submission
        // deadline
        let current_gas_price = self
            .ethereum
            .gas_price()
            .await
            .context("failed to compute current gas price")?;
        let submission_block = self.ethereum.current_block().borrow().number;
        let blocks_until_deadline = submission_deadline.saturating_sub(submission_block);

        // if there is still a tx pending we also have to make sure we outbid that one
        // enough to make the node replace it in the mempool
        let replacement_gas_price = self
            .minimum_replacement_gas_price(mempool, signer, nonce)
            .await;
        let final_gas_price = match &replacement_gas_price {
            Some(replacement_gas_price)
                if replacement_gas_price.max_fee_per_gas > current_gas_price.max_fee_per_gas =>
            {
                *replacement_gas_price
            }
            _ => current_gas_price,
        };

        tracing::debug!(
            submission_block,
            blocks_until_deadline,
            ?replacement_gas_price,
            ?current_gas_price,
            ?final_gas_price,
            ?signer,
            "submitting settlement tx"
        );
        let hash = mempool
            .submit(
                tx.clone(),
                final_gas_price,
                settlement.gas.limit,
                signer,
                nonce,
            )
            .await?;

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
                        submitted_at_block: submission_block.into(),
                        included_in_block: block_number,
                    }),
                    TxStatus::Reverted { block_number } => {
                        return Err(Error::Revert {
                            tx_id: hash.clone(),
                            submitted_at_block: submission_block,
                            reverted_at_block: block_number.into(),
                        })
                    }
                    TxStatus::Pending => {
                        // Check if the current block reached the submission deadline block number
                        if block.number >= submission_deadline {
                            tracing::debug!(
                                submission_deadline,
                                current_block = block.number,
                                settle_tx_hash = ?hash,
                                "exceeded submission deadline, cancelling"
                            );
                            let _ = self
                                .cancel(mempool, final_gas_price, signer, nonce)
                                .await;
                            return Err(Error::Expired {
                                tx_id: hash.clone(),
                                submitted_at_block: submission_block,
                                submission_deadline,
                            });
                        }
                        // Check if transaction still simulates
                        if let Err(err) = self.ethereum.estimate_gas(sim_tx.clone()).await {
                            if err.is_revert() {
                                tracing::info!(
                                    settle_tx_hash = ?hash,
                                    ?err,
                                    "tx started failing in mempool, cancelling"
                                );
                                let _ = self
                                    .cancel(mempool, final_gas_price, signer, nonce)
                                    .await;
                                return Err(Error::SimulationRevert {
                                    submitted_at_block: submission_block,
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
                    submitted_at_block: submission_block.into(),
                });
            }
        }
        result
    }

    /// Cancel a pending tx by sending a zero-value self-transfer at the same
    /// nonce with a higher gas price.
    async fn cancel(
        &self,
        mempool: &infra::mempool::Mempool,
        original_tx_gas_price: Eip1559Estimation,
        signer: eth::Address,
        nonce: u64,
    ) -> Result<TxId, Error> {
        let fallback_gas_price = original_tx_gas_price.scaled_by_pct(GAS_PRICE_BUMP_PCT);
        let replacement_gas_price = self
            .minimum_replacement_gas_price(mempool, signer, nonce)
            .await;

        // the node is the ultimate source of truth to compute the minimum
        // replacement gas price, but if that fails for whatever reason
        // we use our best estimate based on the originally submitted tx
        let final_gas_price = match &replacement_gas_price {
            Some(replacement) => *replacement,
            _ => fallback_gas_price,
        };

        let cancellation = eth::Tx {
            from: signer,
            to: signer,
            value: 0.into(),
            input: Default::default(),
            access_list: Default::default(),
        };

        tracing::debug!(
            ?replacement_gas_price,
            ?fallback_gas_price,
            ?final_gas_price,
            "submitting cancellation tx"
        );

        mempool
            .submit(
                cancellation,
                final_gas_price,
                CANCELLATION_GAS_AMOUNT.into(),
                signer,
                nonce,
            )
            .await
    }

    /// Computes minimum price to replace the last tx that was submitted
    /// with the given nonce. Returns `None` if no tx was submitted with
    /// that nonce yet.
    #[tracing::instrument(skip_all)]
    async fn minimum_replacement_gas_price(
        &self,
        mempool: &infra::Mempool,
        signer: eth::Address,
        next_nonce: u64,
    ) -> Option<Eip1559Estimation> {
        if let Some(last_submission) = mempool.last_submission(signer) {
            if last_submission.nonce == next_nonce {
                Some(last_submission.gas_price.scaled_by_pct(GAS_PRICE_BUMP_PCT))
            } else {
                None
            }
        } else {
            // If we don't have the last submission in-memory (i.e. first submission
            // attempt after a restart) we try to inspect the nodes transaction mempool.
            // This is only done as a backup since it can incur significant latency and
            // is generally not very widely supported.
            let pending_tx = mempool
                .find_pending_tx_in_mempool(signer, next_nonce)
                .await
                .inspect_err(|err| tracing::debug!(?err, "could not inspect tx mempool"))
                .ok()??;

            let pending_tx_gas_price = Eip1559Estimation {
                max_fee_per_gas: pending_tx.max_fee_per_gas(),
                max_priority_fee_per_gas: pending_tx.max_priority_fee_per_gas().or_else(|| {
                    tracing::error!(tx = ?pending_tx.inner.tx_hash(), "pending tx is not EIP 1559");
                    None
                })?,
            };

            Some(pending_tx_gas_price.scaled_by_pct(GAS_PRICE_BUMP_PCT))
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
    #[error(
        "Mined reverted transaction: {tx_id:?}, block number: {reverted_at_block}, submitted at \
         block: {submitted_at_block}"
    )]
    Revert {
        tx_id: eth::TxId,
        submitted_at_block: BlockNo,
        reverted_at_block: BlockNo,
    },
    #[error(
        "Simulation started reverting during submission, block number: {reverted_at_block}, \
         submitted at block: {submitted_at_block}"
    )]
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
