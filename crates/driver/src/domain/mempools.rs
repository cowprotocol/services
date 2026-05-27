use {
    super::competition::solution::{GasFeeOverride, settlement},
    crate::{
        domain::{blockchain::TxStatus, competition::solution::Settlement},
        infra::{self, Ethereum, observe},
    },
    alloy::{consensus::Transaction, eips::eip1559::Eip1559Estimation, primitives::Bytes},
    anyhow::{Context, anyhow},
    eth_domain_types::{self as eth, BlockNo, TxId},
    ethrpc::block_stream::into_stream,
    futures::{FutureExt, StreamExt, future::select_ok},
    itertools::Itertools,
    num::Saturating,
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

/// How the settlement transaction should be submitted on-chain.
#[derive(Debug, Clone)]
pub enum SubmissionMode {
    /// Solver EOA signs and submits directly to the settlement contract.
    Direct(eth::Address),
    /// A dedicated submission EOA signs and pays for the tx while routing it
    /// through the solver's EIP-7702 delegate.
    Delegated {
        /// The address that signs the transaction and whose nonce is used.
        submitter_eoa: eth::Address,
        /// The solver EOA address. In EIP-7702 mode tx.to is set to this
        /// address (which delegates to Solver7702Delegate), instead of the
        /// settlement contract.
        solver_eoa: eth::Address,
    },
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

    /// Race the enabled mempools concurrently; first success wins. Pending
    /// submission futures are dropped at that point and every other mempool is
    /// recorded as `Superseded`. If every mempool fails, return one of the
    /// failure errors.
    pub async fn execute(
        &self,
        settlement: &Settlement,
        submission_deadline: BlockNo,
        mode: &SubmissionMode,
    ) -> Result<eth::TxId, Error> {
        let mut stats = vec![Outcome::Superseded; self.mempools.len()];

        let res = select_ok(self.mempools.iter().zip(stats.iter_mut()).map(
            |(mempool, stat)| {
                async move {
                    let result = self
                        .submit(mempool, settlement, submission_deadline, mode)
                        .instrument(tracing::info_span!("mempool", kind = %mempool))
                        .await;
                    // Log inline so errors from mempools that later get superseded still surface;
                    // metrics are emitted from `update_metrics` once the race outcome is known.
                    observe::mempool_log(mempool, settlement, &result);
                    *stat = Outcome::from(&result);
                    result
                }
                .boxed()
            },
        ))
        .await
        // Drop the remaining futures (and the mutable borrow on `stats` they
        // carry) so `update_metrics` can read `stats` below.
        .map(|(success, _remaining)| success);

        self.update_metrics(&stats);

        Ok(res?.tx_hash)
    }

    /// A mempool is disabled if all of the following are true:
    /// * the settlement may revert (see [`Settlement::may_revert`])
    /// * the pool has revert protection enabled (see
    ///   [`Self::revert_protection`])
    /// * reverts can get mined (see [`infra::Mempool::reverts_can_get_mined`])
    fn is_disabled(&self, mempool: &infra::Mempool, settlement: &Settlement) -> bool {
        settlement.may_revert()
            && matches!(self.revert_protection(), RevertProtection::Enabled)
            && mempool.reverts_can_get_mined()
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
        settlement: &Settlement,
        submission_deadline: BlockNo,
        mode: &SubmissionMode,
    ) -> Result<SubmissionSuccess, Error> {
        if self.is_disabled(mempool, settlement) {
            return Err(Error::Disabled);
        }

        let tx = settlement.transaction(settlement::Internalization::Enable);
        let tx = prepare_submission(tx, mode);
        let signer = tx.from;

        // Instantiate block stream and skip the current block before we submit the
        // settlement. This way we only run iterations in blocks that can potentially
        // include the settlement.
        let mut block_stream = into_stream(self.ethereum.current_block().clone());
        block_stream.next().await;

        let current_block = self.ethereum.current_block().borrow().number;
        // The tx is simulated before submitting the solution to the competition, but a
        // delay between that and the actual execution can cause the simulation to be
        // invalid which doesn't make sense to submit to the mempool anymore.
        if mempool.reverts_can_get_mined() {
            if let Err(err) = self.ethereum.estimate_gas(tx.clone()).await {
                if err.is_revert() {
                    tracing::info!(
                        ?err,
                        "settlement tx simulation reverted before submitting to the mempool"
                    );
                    return Err(Error::SimulationRevert {
                        submitted_at_block: current_block.into(),
                        reverted_at_block: current_block.into(),
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
        let submission_block = self.ethereum.current_block().borrow().number.into();
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

        let final_gas_price = apply_gas_fee_override(
            final_gas_price,
            settlement.gas_fee_override(),
            replacement_gas_price.as_ref(),
        );

        tracing::debug!(
            ?submission_block,
            ?blocks_until_deadline,
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
                        tx_hash: hash,
                        submitted_at_block: submission_block,
                        included_in_block: block_number,
                    }),
                    TxStatus::Reverted { block_number } => {
                        return Err(Error::Revert {
                            tx_id: hash,
                            submitted_at_block: submission_block,
                            reverted_at_block: block_number,
                        })
                    }
                    TxStatus::Pending => {
                        // Check if the current block reached the submission deadline block number
                        if BlockNo(block.number) >= submission_deadline {
                            tracing::debug!(
                                submission_deadline = submission_deadline.0,
                                current_block = block.number,
                                settle_tx_hash = ?hash,
                                "exceeded submission deadline, cancelling"
                            );
                            let _ = self
                                .cancel(mempool, final_gas_price, signer, nonce)
                                .await;
                            return Err(Error::Expired {
                                tx_id: hash,
                                submitted_at_block: submission_block,
                                submission_deadline,
                            });
                        }
                        // Check if transaction still simulates
                        if let Err(err) = self.ethereum.estimate_gas(tx.clone()).await {
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
                                    reverted_at_block: block.number.into(),
                                });
                            } else {
                                tracing::warn!(?hash, ?err, "couldn't re-simulate tx");
                            }
                        }
                    }
                }
            }
            Err(Error::Other(anyhow!(
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
                    submitted_at_block: submission_block,
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

    /// Update per-mempool metrics based on submission outcomes.
    ///
    /// When a winner exists, `Failed` outcomes are reclassified as `Superseded`
    /// since errors are typically race-condition false-positives.
    fn update_metrics(&self, stats: &[Outcome]) {
        let winner_exists = stats.iter().any(|s| matches!(s, Outcome::Success { .. }));
        // Using `zip_eq` to catch regressions in tests (sizes always match in
        // practice).
        for (mempool, &outcome) in self.mempools.iter().zip_eq(stats.iter()) {
            let label = match outcome {
                Outcome::Failed { .. } if winner_exists => Outcome::Superseded.metric_label(),
                other => other.metric_label(),
            };
            observe::mempool_submission_result(mempool, label, outcome.blocks_passed());
        }
    }
}

#[derive(Clone, Copy)]
enum Outcome {
    /// Submission future was dropped because another mempool won the race.
    Superseded,
    Success {
        blocks_passed: u64,
    },
    Failed {
        reason: &'static str,
        blocks_passed: Option<u64>,
    },
    Disabled,
}

impl Outcome {
    fn metric_label(self) -> &'static str {
        match self {
            Outcome::Superseded => "Superseded",
            Outcome::Success { .. } => "Success",
            Outcome::Failed { reason, .. } => reason,
            Outcome::Disabled => "Disabled",
        }
    }

    fn blocks_passed(self) -> Option<u64> {
        match self {
            Outcome::Superseded | Outcome::Disabled => None,
            Outcome::Success { blocks_passed } => Some(blocks_passed),
            Outcome::Failed { blocks_passed, .. } => blocks_passed,
        }
    }
}

impl From<&Result<SubmissionSuccess, Error>> for Outcome {
    fn from(result: &Result<SubmissionSuccess, Error>) -> Self {
        match result {
            Ok(s) => Outcome::Success {
                blocks_passed: s.blocks_passed(),
            },
            Err(Error::Disabled) => Outcome::Disabled,
            Err(err @ (Error::Revert { .. } | Error::SimulationRevert { .. })) => Outcome::Failed {
                reason: "Revert",
                blocks_passed: err.blocks_passed(),
            },
            Err(err @ Error::Expired { .. }) => Outcome::Failed {
                reason: "Expired",
                blocks_passed: err.blocks_passed(),
            },
            Err(Error::Other(_)) => Outcome::Failed {
                reason: "Other",
                blocks_passed: None,
            },
        }
    }
}

/// Applies the solver's gas fee override if present. When a replacement
/// transaction is pending, the solver's values are raised to at least the
/// replacement minimum (a node requirement).
fn apply_gas_fee_override(
    driver_estimate: Eip1559Estimation,
    solver_override: Option<GasFeeOverride>,
    replacement_price: Option<&Eip1559Estimation>,
) -> Eip1559Estimation {
    let Some(gas_override) = solver_override else {
        return driver_estimate;
    };
    let solver_price = Eip1559Estimation {
        max_fee_per_gas: gas_override.max_fee_per_gas,
        max_priority_fee_per_gas: gas_override.max_priority_fee_per_gas,
    };
    match replacement_price {
        Some(replacement) => Eip1559Estimation {
            max_fee_per_gas: std::cmp::max(
                solver_price.max_fee_per_gas,
                replacement.max_fee_per_gas,
            ),
            max_priority_fee_per_gas: std::cmp::max(
                solver_price.max_priority_fee_per_gas,
                replacement.max_priority_fee_per_gas,
            ),
        },
        None => solver_price,
    }
}

/// In EIP-7702 mode, reroute the tx through the solver EOA's delegate. Its
/// fallback expects the 20-byte target address followed by target calldata.
/// `from` is the submitter EOA so simulations see the correct `msg.sender`
/// for the delegate's caller whitelist. The solver EOA is in `tx.to` and
/// becomes `address(this)` when the delegate runs.
fn prepare_submission(tx: &eth::Tx, mode: &SubmissionMode) -> eth::Tx {
    let mut tx = tx.clone();
    match mode {
        SubmissionMode::Direct(solver_eoa) => {
            tx.from = *solver_eoa;
            tx
        }
        SubmissionMode::Delegated {
            submitter_eoa,
            solver_eoa,
        } => {
            let original_target = tx.to;
            tx.from = *submitter_eoa;
            tx.to = *solver_eoa;
            tx.input = delegated_calldata(original_target, &tx.input);
            tx
        }
    }
}

fn delegated_calldata(target: eth::Address, calldata: &Bytes) -> Bytes {
    let mut input = Vec::with_capacity(target.len() + calldata.len());
    input.extend_from_slice(target.as_slice());
    input.extend_from_slice(calldata);
    input.into()
}

pub struct SubmissionSuccess {
    pub tx_hash: eth::TxId,
    /// At which block we started to submit the transaction.
    pub included_in_block: eth::BlockNo,
    /// In which block the transaction actually appeared onchain.
    pub submitted_at_block: eth::BlockNo,
}

impl SubmissionSuccess {
    /// Number of blocks between submission start and on-chain inclusion.
    pub fn blocks_passed(&self) -> u64 {
        self.included_in_block
            .saturating_sub(self.submitted_at_block)
            .0
    }
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

impl Error {
    /// Number of blocks between the first submission and when the error was
    /// returned, if the error carries that timing.
    pub fn blocks_passed(&self) -> Option<u64> {
        let (start, end) = match self {
            Self::Revert {
                submitted_at_block,
                reverted_at_block,
                ..
            }
            | Self::SimulationRevert {
                submitted_at_block,
                reverted_at_block,
            } => (*submitted_at_block, *reverted_at_block),
            Self::Expired {
                submitted_at_block,
                submission_deadline,
                ..
            } => (*submitted_at_block, *submission_deadline),
            Self::Disabled | Self::Other(_) => return None,
        };
        Some(end.saturating_sub(start).0)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{Bytes, address},
    };

    const ORIGINAL_FROM: eth::Address = address!("0000000000000000000000000000000000000001");
    const SETTLEMENT: eth::Address = address!("0000000000000000000000000000000000000002");
    const SOLVER: eth::Address = address!("0000000000000000000000000000000000000003");
    const SUBMITTER: eth::Address = address!("0000000000000000000000000000000000000004");

    fn tx(input: Bytes) -> eth::Tx {
        eth::Tx {
            from: ORIGINAL_FROM,
            to: SETTLEMENT,
            value: 0.into(),
            input,
            access_list: Default::default(),
        }
    }

    #[test]
    fn delegated_submission_rewrites_transaction() {
        let prepared = prepare_submission(
            &tx(Bytes::from_static(&[0xaa, 0xbb])),
            &SubmissionMode::Delegated {
                submitter_eoa: SUBMITTER,
                solver_eoa: SOLVER,
            },
        );
        let mut expected = SETTLEMENT.as_slice().to_vec();
        expected.extend_from_slice(&[0xaa, 0xbb]);

        assert_eq!(prepared.from, SUBMITTER);
        assert_eq!(prepared.to, SOLVER);
        assert_eq!(prepared.input, Bytes::from(expected));
    }
}
