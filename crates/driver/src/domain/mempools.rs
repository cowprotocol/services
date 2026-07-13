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

        // Capture an account-specific failure reported by any mempool so it is
        // not masked by a different error that `select_ok` happens to return
        // last (it yields the LAST error when all futures fail). The caller
        // relies on `SubmitterUnusable` escaping here to bench the account and
        // retry the settlement from another one.
        let account_failure: std::sync::Arc<std::sync::Mutex<Option<AccountFailure>>> =
            Default::default();
        // Set once any mempool actually broadcasts a tx. After that, retrying from
        // another account could double-submit the settlement, so `race_error` must
        // not surface a retryable `SubmitterUnusable` even if a sibling mempool
        // rejected pre-broadcast.
        let any_broadcast = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // Set if any mempool fails before broadcast with an ambiguous error whose tx
        // might still reach the chain (timeout, connection reset, `already known`,
        // `nonce too high`, ...). Unlike a clean account rejection, such a failure
        // means we can't be sure nothing was sent, so it must not trigger a retry.
        let saw_nonretryable = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // Capture a settlement-specific terminal failure (revert/expired) from any
        // lane so `race_error` surfaces the real error, not whatever `select_ok`
        // returns last.
        let terminal: std::sync::Arc<std::sync::Mutex<Option<Error>>> = Default::default();

        let res = select_ok(self.mempools.iter().zip(stats.iter_mut()).map(
            |(mempool, stat)| {
                let account_failure = std::sync::Arc::clone(&account_failure);
                let any_broadcast = std::sync::Arc::clone(&any_broadcast);
                let saw_nonretryable = std::sync::Arc::clone(&saw_nonretryable);
                let terminal = std::sync::Arc::clone(&terminal);
                async move {
                    let result = self
                        .submit(mempool, settlement, submission_deadline, mode, &any_broadcast)
                        .instrument(tracing::info_span!("mempool", kind = %mempool))
                        .await;
                    // Log inline so errors from mempools that later get superseded still surface;
                    // metrics are emitted from `update_metrics` once the race outcome is known.
                    observe::mempool_log(mempool, settlement, &result);
                    match &result {
                        Err(Error::SubmitterUnusable(reason)) => {
                            *account_failure.lock().unwrap() = Some(*reason);
                        }
                        // A disabled mempool was skipped without touching the network.
                        Err(Error::Disabled) => {}
                        Err(err) => match err.terminal() {
                            Some(term) => {
                                terminal.lock().unwrap().get_or_insert(term);
                            }
                            // Any other error (timeout, connection reset, `already
                            // known`, `nonce too high`, ...) may have put a tx on the
                            // wire, so it must block a retry from another account.
                            None => {
                                saw_nonretryable.store(true, std::sync::atomic::Ordering::SeqCst)
                            }
                        },
                        Ok(_) => {}
                    }
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

        match res {
            Ok(success) => Ok(success.tx_hash),
            Err(err) => Err(race_error(
                err,
                terminal.lock().unwrap().take(),
                *account_failure.lock().unwrap(),
                any_broadcast.load(std::sync::atomic::Ordering::SeqCst),
                saw_nonretryable.load(std::sync::atomic::Ordering::SeqCst),
            )),
        }
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
        broadcasted: &std::sync::atomic::AtomicBool,
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

        // Proactively check the signer can cover the gas before broadcasting, so an
        // underfunded account falls back to another one without a wasted submission
        // (issue #4541). A failed balance lookup is not authoritative, so proceed
        // and let the node decide.
        let required_balance = settlement
            .gas
            .required_balance(eth::U256::from(final_gas_price.max_fee_per_gas));
        match self.ethereum.balance(signer).await {
            Ok(balance) if balance < required_balance => {
                tracing::warn!(
                    ?signer,
                    ?balance,
                    ?required_balance,
                    "submission account balance too low for gas, falling back"
                );
                return Err(Error::SubmitterUnusable(AccountFailure::InsufficientFunds));
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(
                ?signer,
                ?err,
                "could not check submission account balance before submitting"
            ),
        }

        let hash = mempool
            .submit(
                tx.clone(),
                final_gas_price,
                settlement.gas.limit,
                signer,
                nonce,
            )
            .await?;
        // The tx is now on the wire. Record it so a sibling mempool's pre-broadcast
        // account failure can't trigger a retry that double-submits this settlement.
        broadcasted.store(true, std::sync::atomic::Ordering::SeqCst);

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
                        // A node can report a new block before its receipt index catches up,
                        // so re-simulating reverts on our own already-applied tx. Tell that
                        // apart from a real revert with two signals: our `Settlement` event in
                        // any block since submission (which `eth_getLogs` sees even while the
                        // receipt lags), and the signer's pending nonce (which still covers our
                        // tx while it sits in the mempool).
                        let (gas, mined, pending_nonce) = futures::join!(
                            self.ethereum.estimate_gas(tx.clone()),
                            self.ethereum
                                .successful_settlement_block(hash, submission_block.0),
                            self.ethereum.pending_transaction_count(signer),
                        );

                        if let Ok(Some(included_in_block)) = mined {
                            // Found on-chain in [submission_block, head]. The receipt-by-hash
                            // lookup is just lagging, and scanning the whole range catches it
                            // even if the block stream coalesced past the block it mined in.
                            // Report success now rather than wait on the receipt and risk the
                            // deadline.
                            tracing::info!(
                                ?hash,
                                ?included_in_block,
                                "settlement found on-chain via getLogs, treating as success"
                            );
                            return Ok(SubmissionSuccess {
                                tx_hash: hash,
                                submitted_at_block: submission_block,
                                included_in_block,
                            });
                        }

                        let resimulation_reverted = matches!(&gas, Err(err) if err.is_revert());
                        if requires_cancellation(resimulation_reverted, &pending_nonce, nonce) {
                            tracing::info!(
                                settle_tx_hash = ?hash,
                                err = ?gas.as_ref().err(),
                                "tx started failing in mempool, cancelling"
                            );
                            let _ = self.cancel(mempool, final_gas_price, signer, nonce).await;
                            return Err(Error::SimulationRevert {
                                submitted_at_block: submission_block,
                                reverted_at_block: block.number.into(),
                            });
                        } else if let Err(err) = &pending_nonce {
                            tracing::warn!(
                                ?hash,
                                ?err,
                                "couldn't fetch the pending nonce, not cancelling"
                            );
                        } else if resimulation_reverted {
                            tracing::debug!(
                                ?hash,
                                "detected false positive revert (already pending tx conflicted \
                                 with our simulation), waiting for next block"
                            );
                        } else if let Err(err) = &gas {
                            tracing::warn!(?hash, ?err, "couldn't re-simulate tx");
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
            // One last check in case the tx landed after the loop exited, e.g. the
            // receipt finally caught up to a block we had already processed.
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
            Err(Error::SubmitterUnusable(_)) => Outcome::Failed {
                reason: "SubmitterUnusable",
                blocks_passed: None,
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
    /// In which block the transaction actually appeared onchain.
    pub included_in_block: eth::BlockNo,
    /// At which block we started to submit the transaction.
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
    /// The submission account could not broadcast the transaction for a reason
    /// specific to that account (e.g. insufficient gas funds, stale nonce, a
    /// pending tx that can't be replaced). Nothing was broadcast, so the same
    /// settlement can safely be retried from a different account.
    #[error("submission account unusable: {0}")]
    SubmitterUnusable(AccountFailure),
    #[error("Failed to submit: {0:?}")]
    Other(#[from] anyhow::Error),
}

/// Account-specific reasons a node rejects a transaction at submission time,
/// before it is broadcast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountFailure {
    InsufficientFunds,
    Nonce,
    ReplacementUnderpriced,
}

impl AccountFailure {
    /// Stable label for metrics/logging.
    pub fn as_str(self) -> &'static str {
        match self {
            AccountFailure::InsufficientFunds => "insufficient_funds",
            AccountFailure::Nonce => "nonce",
            AccountFailure::ReplacementUnderpriced => "replacement_underpriced",
        }
    }
}

impl std::fmt::Display for AccountFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classify a failed `eth_sendRawTransaction` by its error message. Returns
/// `Some` only for failures specific to the *sending account*; transaction-
/// and node-level failures return `None` so they keep their existing handling.
///
/// `already known` is intentionally excluded: it means our exact transaction is
/// already in the mempool and may still be mined, so it is not safe to retry
/// from another account.
pub fn classify_submission_failure(message: &str) -> Option<AccountFailure> {
    let message = message.to_lowercase();
    // Match on keyword pairs rather than exact phrases so non-Geth clients are
    // also covered (e.g. Nethermind's `SenderInsufficientFunds` / `NonceTooLow`,
    // which omit the spaces Geth uses).
    if message.contains("insufficient") && message.contains("funds") {
        Some(AccountFailure::InsufficientFunds)
    } else if message.contains("nonce") && message.contains("low") {
        Some(AccountFailure::Nonce)
    } else if message.contains("replacement") && message.contains("underpriced") {
        // Only *replacement* underpricing is account-specific (a stuck nonce that
        // another account avoids). A plain/global "underpriced" means the fee is
        // below the node's minimum, which every account hits the same way, so it
        // must not be benched or retried.
        Some(AccountFailure::ReplacementUnderpriced)
    } else {
        None
    }
}

/// Choose which error to surface when the mempool race produced no success.
///
/// `select_ok` yields whichever error finished last, so a `SubmitterUnusable`
/// reported by one mempool can be masked by a generic error (e.g. a timeout)
/// from another. The caller benches the account and retries on
/// `SubmitterUnusable`, so surface it whenever a mempool reported one, unless
/// one of the following holds: `terminal` is set, meaning some lane hit a
/// settlement-specific terminal failure (revert/expired), which is
/// authoritative; `broadcasted` is set, meaning some mempool already put a tx
/// on the wire and retrying could double-submit; or `saw_nonretryable` is set,
/// meaning a mempool failed before broadcast with an ambiguous error (timeout,
/// connection reset, `already known`, `nonce too high`) whose tx might still be
/// live, which is likewise unsafe to retry.
fn race_error(
    last_error: Error,
    terminal: Option<Error>,
    account_failure: Option<AccountFailure>,
    broadcasted: bool,
    saw_nonretryable: bool,
) -> Error {
    // A terminal failure (revert/expired) from any lane is authoritative and
    // never retryable, so it always wins the race, whichever lane's error
    // `select_ok` returns last.
    if let Some(terminal) = terminal {
        return terminal;
    }
    // Once a tx is on the wire, never surface a retryable account failure:
    // downgrade even a `SubmitterUnusable` that `select_ok` happened to return
    // last, since a retry from another account could double-submit the
    // settlement.
    if broadcasted {
        return match last_error {
            Error::SubmitterUnusable(reason) => Error::Other(anyhow!(
                "submission account unusable ({reason}) after a transaction was already broadcast"
            )),
            other => other,
        };
    }
    match account_failure {
        // Only retry when every active mempool failed cleanly before sending. If a
        // sibling lane returned an ambiguous error, its tx might still be on the
        // wire, so retrying from another account could double-submit; keep the real
        // error instead, stripping a `SubmitterUnusable` that `select_ok` happened
        // to return last so it can't leak a retry.
        Some(reason) if !saw_nonretryable => Error::SubmitterUnusable(reason),
        Some(reason) => match last_error {
            Error::SubmitterUnusable(_) => Error::Other(anyhow!(
                "submission account unusable ({reason}) alongside an ambiguous failure from \
                 another mempool; not retrying"
            )),
            other => other,
        },
        None => last_error,
    }
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
            Self::Disabled | Self::SubmitterUnusable(_) | Self::Other(_) => return None,
        };
        Some(end.saturating_sub(start).0)
    }

    /// A standalone copy of this error if it is a settlement-specific terminal
    /// failure (revert/expired), else `None`. `Error` isn't `Clone` (the
    /// `Other(anyhow::Error)` variant), but the terminal variants carry only
    /// `Copy` fields.
    fn terminal(&self) -> Option<Error> {
        match self {
            Error::Revert {
                tx_id,
                submitted_at_block,
                reverted_at_block,
            } => Some(Error::Revert {
                tx_id: *tx_id,
                submitted_at_block: *submitted_at_block,
                reverted_at_block: *reverted_at_block,
            }),
            Error::SimulationRevert {
                submitted_at_block,
                reverted_at_block,
            } => Some(Error::SimulationRevert {
                submitted_at_block: *submitted_at_block,
                reverted_at_block: *reverted_at_block,
            }),
            Error::Expired {
                tx_id,
                submitted_at_block,
                submission_deadline,
            } => Some(Error::Expired {
                tx_id: *tx_id,
                submitted_at_block: *submitted_at_block,
                submission_deadline: *submission_deadline,
            }),
            Error::Disabled | Error::SubmitterUnusable(_) | Error::Other(_) => None,
        }
    }
}

/// Whether a submitted settlement whose receipt is missing should be cancelled.
/// We cancel only when the re-simulation reverts and the signer's pending nonce
/// shows our tx is gone from the mempool (`pending_nonce <= submission_nonce`,
/// so it was neither mined nor is it still queued). While the tx is still
/// queued the revert is just our own pending tx re-applied, a false positive,
/// so we keep waiting. A failed nonce lookup counts as "unknown" and never
/// cancels.
fn requires_cancellation<E>(
    resimulation_reverted: bool,
    pending_nonce: &Result<u64, E>,
    submission_nonce: u64,
) -> bool {
    resimulation_reverted && matches!(pending_nonce, Ok(n) if *n <= submission_nonce)
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

    const SUBMISSION_NONCE: u64 = 7;
    const STILL_QUEUED: Result<u64, ()> = Ok(8); // pending advanced past our nonce
    const DROPPED: Result<u64, ()> = Ok(7); // pending still at our nonce, tx is gone
    const NONCE_LOOKUP_FAILED: Result<u64, ()> = Err(());

    #[test]
    fn cancels_only_when_reverted_and_tx_left_the_mempool() {
        // Reverted and the tx is no longer queued: a real revert, cancel.
        assert!(requires_cancellation(true, &DROPPED, SUBMISSION_NONCE));
        // Reverted but still queued: the revert is our own pending tx, so wait.
        assert!(!requires_cancellation(
            true,
            &STILL_QUEUED,
            SUBMISSION_NONCE
        ));
        // Re-simulation still succeeds: wait regardless of the nonce.
        assert!(!requires_cancellation(false, &DROPPED, SUBMISSION_NONCE));
    }

    #[test]
    fn failed_nonce_lookup_never_cancels() {
        // An unknown pending nonce must never trigger a cancel, even on a revert.
        // Otherwise a flaky nonce lookup reverts to the original bug: cancelling a
        // tx that may have mined or is still queued.
        assert!(!requires_cancellation(
            true,
            &NONCE_LOOKUP_FAILED,
            SUBMISSION_NONCE
        ));
    }

    #[test]
    fn classifies_account_specific_submission_failures() {
        use AccountFailure::*;
        // Geth-family messages, with the surrounding wrapper text nodes add.
        assert_eq!(
            classify_submission_failure(
                "server returned an error response: error code -32000: insufficient funds for gas \
                 * price + value"
            ),
            Some(InsufficientFunds)
        );
        assert_eq!(
            classify_submission_failure("error code -32000: nonce too low"),
            Some(Nonce)
        );
        assert_eq!(
            classify_submission_failure("replacement transaction underpriced"),
            Some(ReplacementUnderpriced)
        );
        // Case-insensitive.
        assert_eq!(
            classify_submission_failure("Insufficient Funds For Transfer"),
            Some(InsufficientFunds)
        );
    }

    #[test]
    fn does_not_classify_non_account_failures_as_account_specific() {
        // Settlement-level and node-level failures must NOT be retried from a
        // different account.
        assert_eq!(classify_submission_failure("execution reverted"), None);
        // `already known` means our exact tx is already pending and may mine;
        // retrying elsewhere could double-submit, so it must not be classified.
        assert_eq!(classify_submission_failure("already known"), None);
        assert_eq!(classify_submission_failure("too many requests"), None);
        assert_eq!(
            classify_submission_failure("connection reset by peer"),
            None
        );
        // `nonce too high` is a gap (the tx gets queued), not a clean
        // rejection, so it must not be treated as retryable.
        assert_eq!(classify_submission_failure("nonce too high"), None);
        // Plain/global underpricing is a node/network fee-floor rejection, not
        // account-specific: another account uses the same fee path, so retrying
        // would just re-fail and bench every submitter.
        assert_eq!(classify_submission_failure("transaction underpriced"), None);
        assert_eq!(
            classify_submission_failure("max fee per gas less than block base fee"),
            None
        );
    }

    #[test]
    fn classifies_non_geth_client_wording() {
        use AccountFailure::*;
        // Clients such as Nethermind use spaceless variants; keyword-pair
        // matching still classifies them.
        assert_eq!(
            classify_submission_failure("SenderInsufficientFunds"),
            Some(InsufficientFunds)
        );
        assert_eq!(classify_submission_failure("NonceTooLow"), Some(Nonce));
        assert_eq!(
            classify_submission_failure("ReplacementTransactionUnderpriced"),
            Some(ReplacementUnderpriced)
        );
    }

    #[test]
    fn account_failure_is_surfaced_over_a_masking_race_error() {
        use AccountFailure::*;
        // Pre-broadcast, every lane failed cleanly: the account-specific failure is
        // surfaced so the settlement is benched and retried (issue #4541), even when
        // `select_ok` returns a different clean error (here `Disabled`) last.
        assert!(matches!(
            race_error(Error::Disabled, None, Some(Nonce), false, false),
            Error::SubmitterUnusable(Nonce)
        ));
        // ...and when the account failure is itself the error returned last.
        assert!(matches!(
            race_error(
                Error::SubmitterUnusable(Nonce),
                None,
                Some(Nonce),
                false,
                false
            ),
            Error::SubmitterUnusable(Nonce)
        ));
        // Pre-broadcast but a sibling lane returned an ambiguous error (timeout,
        // connection reset, `already known`, `nonce too high`): its tx might still be
        // on the wire, so the account failure must NOT be surfaced, or the settlement
        // could be retried from another EOA and double-submitted.
        assert!(matches!(
            race_error(
                Error::Other(anyhow!("connection reset")),
                None,
                Some(Nonce),
                false,
                true,
            ),
            Error::Other(_)
        ));
        // Even when the account failure is the error returned last, an ambiguous
        // sibling downgrades it so the caller does not retry.
        assert!(matches!(
            race_error(
                Error::SubmitterUnusable(Nonce),
                None,
                Some(Nonce),
                false,
                true
            ),
            Error::Other(_)
        ));
        // With no account-specific failure, the original error is preserved.
        assert!(matches!(
            race_error(Error::Disabled, None, None, false, false),
            Error::Disabled
        ));
        // A terminal failure (revert/expired) from any lane wins the race, even
        // when `select_ok` returns a `SubmitterUnusable` from another lane last.
        // Otherwise the terminal reason degrades to `Other` and the solver sees
        // `Fail` instead of the revert.
        assert!(matches!(
            race_error(
                Error::SubmitterUnusable(Nonce),
                Some(Error::SimulationRevert {
                    submitted_at_block: BlockNo(1),
                    reverted_at_block: BlockNo(2),
                }),
                Some(InsufficientFunds),
                false,
                true,
            ),
            Error::SimulationRevert { .. }
        ));
        // A terminal failure stays authoritative even once another lane broadcast:
        // a terminal error never triggers a retry, so surfacing it is safe.
        assert!(matches!(
            race_error(
                Error::Other(anyhow!("Block stream finished unexpectedly")),
                Some(Error::SimulationRevert {
                    submitted_at_block: BlockNo(1),
                    reverted_at_block: BlockNo(2),
                }),
                None,
                true,
                false,
            ),
            Error::SimulationRevert { .. }
        ));
        // Post-broadcast: once a mempool put a tx on the wire, an account failure
        // another mempool reported must NOT be surfaced, or the settlement could be
        // retried from another EOA and double-submitted.
        assert!(matches!(
            race_error(
                Error::Other(anyhow!("Block stream finished unexpectedly")),
                None,
                Some(Nonce),
                true,
                false,
            ),
            Error::Other(_)
        ));
        // Even when the last error is itself the pre-broadcast account failure, a
        // broadcast elsewhere downgrades it so the caller does not retry.
        assert!(matches!(
            race_error(
                Error::SubmitterUnusable(Nonce),
                None,
                Some(Nonce),
                true,
                false
            ),
            Error::Other(_)
        ));
    }
}
