//! One `Competition` per solver engine, mounted on the API under `/{name}`.

use {
    super::{Auction, Order, auction::Id, solution::Solution},
    crate::infra::{blockchain::Solana, solver::Solver},
    cow_settlement_interface::SettlementError,
    itertools::Itertools,
    moka::sync::Cache,
    solana_sdk::{
        instruction::InstructionError,
        pubkey::Pubkey,
        signature::Signature,
        transaction::{TransactionError, VersionedTransaction},
    },
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
    tracing::Instrument,
};

const SOLUTION_CACHE_TTL: Duration = Duration::from_secs(60);
/// Target Solana slot duration in milliseconds, used to turn a submission
/// deadline slot into a wall-clock confirmation timeout. This is mainnet's
/// target; other clusters can drift.
const SLOT_DURATION_MS: u64 = 400;
/// The network's per-transaction byte ceiling,
/// `solana_packet::PACKET_DATA_SIZE` without the dependency. An RPC node
/// rejects a larger transaction before it simulates anything.
const MAX_TRANSACTION_BYTES: u64 = 1232;

/// Cache key for a proposed solution.
///
/// The engine assigns `solution_id` and may repeat ids across auctions, so
/// the key needs the autopilot-assigned `auction_id` too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    auction_id: Id,
    solution_id: u64,
}

/// All solutions from one `solve` call share the same auction, hence the
/// `Arc`.
#[derive(Clone)]
struct CachedSolution {
    auction: Arc<Auction>,
    solution: Solution,
}

pub(crate) struct Competition {
    solver: Solver,
    blockchain: Arc<Solana>,
    solutions: Cache<Key, CachedSolution>,
}

impl Competition {
    pub fn new(solver: Solver, blockchain: Arc<Solana>) -> Self {
        Self {
            solver,
            blockchain,
            solutions: Cache::builder().time_to_live(SOLUTION_CACHE_TTL).build(),
        }
    }

    pub fn solver_name(&self) -> &str {
        self.solver.name()
    }

    /// Solve the auction and cache each solution for a later `settle`.
    pub async fn solve(&self, auction_id: Id, auction: &Auction) -> Result<Vec<Solution>, Error> {
        let solutions = self.compute_solutions(auction).await?;

        let auction = Arc::new(auction.clone());
        for solution in &solutions {
            self.solutions.insert(
                Key {
                    auction_id,
                    solution_id: solution.id,
                },
                CachedSolution {
                    auction: Arc::clone(&auction),
                    solution: solution.clone(),
                },
            );
        }

        Ok(solutions)
    }

    /// Send the auction to the solver engine and return its deduplicated
    /// solutions without caching them.
    pub async fn compute_solutions(&self, auction: &Auction) -> Result<Vec<Solution>, Error> {
        let solutions = self
            .solver
            .solve(auction, self.blockchain.program_id())
            .await?;

        // Discard solutions with duplicate ids. The first occurrence wins, and
        // `unique_by` keeps response order. The engine may repeat ids across
        // requests, so this check covers the current response and not the
        // cache.
        let total = solutions.len();
        let solutions: Vec<Solution> = solutions.into_iter().unique_by(|s| s.id).collect();
        if solutions.len() < total {
            tracing::warn!(
                solver = %self.solver.name(),
                discarded = total - solutions.len(),
                "discarding solutions with duplicate ids"
            );
        }
        Ok(solutions)
    }

    /// Submit a previously proposed solution on chain.
    ///
    /// The work runs on a spawned task: a client disconnect must not cancel
    /// a settlement mid-flight, and a dropped join handle detaches the task
    /// to run to completion. An abort between consuming the solution and the
    /// send would destroy the solution without submitting; an abort mid-send
    /// would lose the returned signature. The task terminates on its own:
    /// each RPC request has the client's timeout, and the confirmation loop
    /// exits at the latest when the blockhash expires (~150 slots), after
    /// which the transaction can no longer land.
    ///
    /// A successful return means the transaction reached the cluster at the
    /// RPC client's configured commitment level. This makes the Solana
    /// driver's 200 semantics match the EVM driver's: the response is only
    /// returned after the transaction is confirmed on-chain. The response is
    /// informational; the indexer remains the authority on settlement state.
    pub async fn settle(
        self: &Arc<Self>,
        auction_id: Id,
        solution_id: u64,
        submission_deadline_slot: u64,
        creations: Vec<VersionedTransaction>,
    ) -> Result<Signature, Error> {
        let this = Arc::clone(self);
        let task = tokio::spawn(
            async move {
                let result = this
                    .process_settle_request(
                        auction_id,
                        solution_id,
                        submission_deadline_slot,
                        creations,
                    )
                    .await;
                match &result {
                    Ok(signature) => tracing::info!(%signature, "settlement submitted"),
                    Err(error) => tracing::warn!(?error, "settle failed"),
                }
                metrics()
                    .outcomes
                    .with_label_values(&[outcome_label(&result), this.solver.name()])
                    .inc();
                result
            }
            .instrument(tracing::info_span!("settle", ?auction_id, solution_id)),
        );
        task.await.unwrap_or_else(|error| {
            tracing::error!(?error, "settle task panicked");
            // The task's own counting unwound with the panic, so the panic
            // is counted here. Every other outcome counts inside the task,
            // which keeps counting when a disconnect detaches it.
            let result = Err(Error::TaskPanicked);
            metrics()
                .outcomes
                .with_label_values(&[outcome_label(&result), self.solver.name()])
                .inc();
            result
        })
    }

    /// The settlement worker spawned by [`settle`](Self::settle).
    ///
    /// A send failure does not restore the solution: the transaction may
    /// have reached the network despite the error, and a retry could settle
    /// twice.
    ///
    /// Deferred work:
    /// - admission semaphore(1),
    /// - pre-submission simulation,
    /// - ALT caching,
    /// - re-sending / retry loop.
    async fn process_settle_request(
        self: &Arc<Self>,
        auction_id: Id,
        solution_id: u64,
        submission_deadline_slot: u64,
        creations: Vec<VersionedTransaction>,
    ) -> Result<Signature, Error> {
        let key = Key {
            auction_id,
            solution_id,
        };
        let CachedSolution { auction, solution } = self
            .solutions
            .get(&key)
            .ok_or(Error::SolutionNotAvailable)?;
        let cu_estimate = solution.cu_estimate;

        let current_slot = self.blockchain.slot().await.map_err(Error::Rpc)?;
        if current_slot >= submission_deadline_slot {
            return Err(Error::DeadlineExceeded);
        }
        let deadline = Instant::now()
            + Duration::from_millis(
                (submission_deadline_slot - current_slot).saturating_mul(SLOT_DURATION_MS),
            );

        // TODO: admission semaphore(1).

        let program_id = self.blockchain.program_id();

        let orders = orders_with_trades(auction.orders.clone(), &solution);
        tracing::debug!(
            orders = ?orders.iter().map(|order| order.uid.to_string()).collect::<Vec<_>>(),
            "settling orders"
        );

        let settlement = super::Settlement::new(program_id, auction_id, orders, solution)?;

        // Land the creations only after the solution validated: an invalid
        // solution must not cost the funder any fees. They get the full
        // remaining window: the settlement cannot run without them, so
        // reserving time for it would only waste attempts, and the
        // zero-timeout guard below aborts retryably when nothing remains.
        self.land_creations(&creations, deadline).await?;

        let resolved = settlement
            .resolve_accounts(&self.blockchain, self.solver.pubkey())
            .await?;

        let latest = self
            .blockchain
            .latest_confirmed_blockhash()
            .await
            .map_err(Error::Rpc)?;
        let transaction = resolved.encode(self.solver.keypair(), latest.blockhash)?;
        if let Some(size) = observe_transaction(&transaction, cu_estimate)
            && size > MAX_TRANSACTION_BYTES
        {
            return Err(Error::TransactionTooLarge { size });
        }

        self.simulate_settlement(&transaction).await?;

        // A zero timeout still polls the send future once, which could
        // submit the transaction past the deadline, so handle it before the
        // solution is consumed and while a retry is still possible.
        let confirm_timeout = deadline.saturating_duration_since(Instant::now());
        if confirm_timeout.is_zero() {
            return Err(Error::DeadlineExceeded);
        }

        // Consume the entry only now, when the transaction is about to reach
        // the network. One atomic removal takes the chosen solution. A
        // concurrent `/settle` for it then observes a missing entry and
        // cannot settle the solution again. The auction's other solutions
        // stay in the cache because the autopilot can award several winners
        // per auction. Those winners have disjoint token pairs, so they
        // cannot share an order.
        if self.solutions.remove(&key).is_none() {
            return Err(Error::SolutionNotAvailable);
        }

        // The driver signs the transaction, so the signature is known before
        // the send. A confirmation that never returns must still leave it in
        // the logs.
        if let Some(signature) = transaction.signatures.first() {
            tracing::info!(%signature, "submitting settlement");
        }

        // TODO: a provably unsent transaction (connect failure at send time)
        // loses the solution here; restore the cache entry on that class. Needs
        // the send/confirm split in cow-solana-rpc (planned follow-up PR).
        let signature = tokio::time::timeout(
            confirm_timeout,
            self.blockchain.send_and_confirm_transaction(&transaction),
        )
        .await
        .map_err(|_| {
            if let Some(signature) = transaction.signatures.first() {
                tracing::warn!(
                    %signature,
                    "confirmation timed out, the transaction may still land"
                );
            }
            Error::DeadlineExceeded
        })?
        .map_err(|err| Error::FailedToSubmit {
            settlement_error: err
                .get_transaction_error()
                .and_then(|err| settlement_error(program_id, &transaction, &err)),
            err,
        })?;

        Ok(signature)
    }

    /// Land the sponsored creation transactions before `deadline`. The
    /// settlement needs them confirmed first: `BeginSettle` reads the order
    /// PDAs, and the simulation runs against confirmed state. The
    /// transactions are independent, so they land concurrently. A send
    /// failure whose signature the cluster already knows means an earlier
    /// attempt landed the creation, which is success. A creation that landed
    /// but failed on chain also counts as known: the settlement then fails
    /// at the simulation over the missing order PDA.
    async fn land_creations(
        &self,
        creations: &[VersionedTransaction],
        deadline: Instant,
    ) -> Result<(), Error> {
        if creations.is_empty() {
            return Ok(());
        }
        let timeout = deadline.saturating_duration_since(Instant::now());
        if timeout.is_zero() {
            return Err(Error::DeadlineExceeded);
        }
        futures::future::try_join_all(creations.iter().map(|creation| async move {
            let sent = tokio::time::timeout(
                timeout,
                self.blockchain.send_and_confirm_transaction(creation),
            )
            .await
            .map_err(|_| {
                if let Some(signature) = creation.signatures.first() {
                    tracing::warn!(
                        %signature,
                        "creation confirmation timed out, the transaction may still land"
                    );
                }
                Error::DeadlineExceeded
            })?;
            match sent {
                Ok(signature) => {
                    tracing::info!(%signature, "order creation submitted");
                    Ok(())
                }
                Err(error) => {
                    // A failed status check must not mask the send failure:
                    // treat it as not landed and surface the original error.
                    let landed = match creation.signatures.first() {
                        Some(signature) => self
                            .blockchain
                            .known_signatures(&[*signature])
                            .await
                            .ok()
                            .and_then(|known| known.first().copied())
                            .unwrap_or(false),
                        None => false,
                    };
                    if landed {
                        Ok(())
                    } else {
                        Err(Error::FailedToCreate(error))
                    }
                }
            }
        }))
        .await?;
        Ok(())
    }

    /// Simulate a settlement transaction before sending it.
    async fn simulate_settlement(&self, transaction: &VersionedTransaction) -> Result<(), Error> {
        tracing::debug!("simulating settlement transaction");
        let simulation = self
            .blockchain
            .simulate_transaction(transaction)
            .await
            .map_err(Error::Rpc)?;
        if let Some(err) = &simulation.err {
            // Only the program logs surface here, the error itself carries
            // the failure and its decoded settlement error to the settle
            // task's log.
            tracing::warn!(logs = ?simulation.logs, "settlement simulation failed");
            return Err(Error::SimulationFailed {
                settlement_error: settlement_error(
                    self.blockchain.program_id(),
                    transaction,
                    &err.clone().into(),
                ),
                err: err.clone(),
            });
        }
        tracing::debug!("settlement simulation passed");
        Ok(())
    }
}

/// The settlement program's own error behind a failed transaction. Only the
/// failing instruction's owner can interpret a custom code: a foreign
/// program's code (a Jupiter route, for example) must not be read as ours,
/// and a code newer than the interface crate decodes to nothing.
fn settlement_error(
    program_id: Pubkey,
    transaction: &VersionedTransaction,
    err: &TransactionError,
) -> Option<SettlementError> {
    let TransactionError::InstructionError(index, InstructionError::Custom(code)) = err else {
        return None;
    };
    let message = &transaction.message;
    let instruction = message.instructions().get(usize::from(*index))?;
    let program = message
        .static_account_keys()
        .get(usize::from(instruction.program_id_index))?;
    (*program == program_id)
        .then(|| SettlementError::try_from(*code).ok())
        .flatten()
}

/// The program settles exactly the orders passed to `BeginSettle`, so the
/// orders the solution does not fill must stay out of the settlement.
fn orders_with_trades(orders: Vec<Order>, solution: &Solution) -> Vec<Order> {
    orders
        .into_iter()
        .filter(|order| {
            solution
                .trades
                .iter()
                .any(|trade| trade.order_uid == order.uid)
        })
        .collect()
}

/// An error the competition reports to the API layer.
#[derive(Debug, thiserror::Error)]
#[expect(
    dead_code,
    reason = "TooManyPendingSettlements is pending the deferred admission semaphore check"
)]
pub(crate) enum Error {
    #[error("solver engine failed: {0}")]
    Solver(#[from] crate::infra::solver::Error),
    /// Never solved, or already settled.
    #[error("solution not available")]
    SolutionNotAvailable,
    #[error("submission deadline slot exceeded")]
    DeadlineExceeded,
    #[error("too many pending settlements")]
    TooManyPendingSettlements,
    /// A pre-submission RPC read failed; nothing was submitted.
    #[error("rpc request failed: {0}")]
    Rpc(#[source] cow_solana_rpc::Error),
    #[error("failed to submit or confirm settlement: {err}, settlement error {settlement_error:?}")]
    FailedToSubmit {
        #[source]
        err: cow_solana_rpc::Error,
        settlement_error: Option<SettlementError>,
    },
    #[error("failed to submit or confirm an order creation: {0}")]
    FailedToCreate(#[source] cow_solana_rpc::Error),
    /// The pre-submission simulation failed. The transaction was not sent.
    #[error("settlement simulation failed: {err}, settlement error {settlement_error:?}")]
    SimulationFailed {
        #[source]
        err: cow_solana_rpc::UiTransactionError,
        settlement_error: Option<SettlementError>,
    },
    /// The encoded settlement exceeds the network's per-transaction ceiling.
    /// Nothing was sent.
    #[error("settlement transaction is {size} bytes, over the {MAX_TRANSACTION_BYTES} limit")]
    TransactionTooLarge { size: u64 },
    #[error("failed to resolve settlement accounts: {0}")]
    Resolve(#[from] super::settlement::ResolveError),
    #[error("failed to encode settlement: {0}")]
    Settlement(#[from] super::settlement::Error),
    /// The driver does not know whether the transaction reached the network.
    #[error("settle task panicked")]
    TaskPanicked,
}

/// Per-settlement observability: attempt outcomes and the built transaction's
/// footprint against the network's per-transaction ceilings.
#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "settlement")]
struct Metrics {
    /// Settlement attempts by final outcome and solver.
    #[metric(labels("outcome", "solver"))]
    outcomes: prometheus::IntCounterVec,
    /// Serialized settlement transaction size in bytes. The network rejects a
    /// transaction over 1232 bytes.
    #[metric(buckets(600., 800., 1000., 1100., 1200., 1232., 1400., 1600.))]
    transaction_bytes: prometheus::Histogram,
    /// Settlement transaction account count, static keys plus lookup-table
    /// loaded. The runtime caps a transaction at 64.
    #[metric(buckets(16., 24., 32., 40., 48., 56., 64., 80.))]
    transaction_accounts: prometheus::Histogram,
    /// Solver-estimated compute-unit limit for the settlement. The maximum is
    /// 1.4M per transaction.
    #[metric(buckets(
        100_000., 200_000., 400_000., 800_000., 1_000_000., 1_200_000., 1_400_000.
    ))]
    compute_units: prometheus::Histogram,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}

/// Record the built transaction's footprint against the per-transaction bytes,
/// account, and compute-unit ceilings, and hand back the wire size it measured.
fn observe_transaction(
    transaction: &VersionedTransaction,
    cu_estimate: Option<u32>,
) -> Option<u64> {
    let metrics = metrics();
    let bytes = encoded_size(transaction);
    if let Some(bytes) = bytes {
        metrics.transaction_bytes.observe(bytes as f64);
    }
    metrics
        .transaction_accounts
        .observe(account_count(transaction) as f64);
    if let Some(cu) = cu_estimate {
        metrics.compute_units.observe(f64::from(cu));
    }
    bytes
}

/// The transaction's wire size, `None` when it does not serialize.
fn encoded_size(transaction: &VersionedTransaction) -> Option<u64> {
    bincode::serialized_size(transaction).ok()
}

/// Total accounts a transaction resolves to: its static keys plus every
/// address loaded from its lookup tables.
fn account_count(transaction: &VersionedTransaction) -> usize {
    let message = &transaction.message;
    let loaded = message
        .address_table_lookups()
        .map(|lookups| {
            lookups
                .iter()
                .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    message.static_account_keys().len() + loaded
}

/// The metrics label for a finished settlement attempt.
fn outcome_label(result: &Result<Signature, Error>) -> &'static str {
    let error = match result {
        Ok(_) => return "submitted",
        Err(error) => error,
    };
    match error {
        Error::Solver(_) => "solver_failed",
        Error::SolutionNotAvailable => "solution_unavailable",
        Error::DeadlineExceeded => "deadline_exceeded",
        Error::TooManyPendingSettlements => "throttled",
        Error::Rpc(_) => "rpc_failed",
        Error::FailedToSubmit { .. } => "submit_failed",
        Error::FailedToCreate(_) => "creation_failed",
        Error::SimulationFailed { .. } => "simulation_failed",
        Error::TransactionTooLarge { .. } => "transaction_too_large",
        Error::Resolve(_) => "resolve_failed",
        Error::Settlement(_) => "invalid_settlement",
        Error::TaskPanicked => "panicked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a custom code from the settlement program's own instruction
    /// decodes: a foreign program's code, an unknown code, and a non-custom
    /// error read as nothing.
    #[test]
    fn decodes_only_own_custom_codes() {
        let ours = Pubkey::new_unique();
        let foreign = Pubkey::new_unique();
        let message = solana_sdk::message::Message::new(
            &[
                solana_sdk::instruction::Instruction::new_with_bytes(foreign, &[], vec![]),
                solana_sdk::instruction::Instruction::new_with_bytes(ours, &[], vec![]),
            ],
            Some(&Pubkey::new_unique()),
        );
        let transaction = VersionedTransaction {
            signatures: vec![],
            message: solana_sdk::message::VersionedMessage::Legacy(message),
        };
        let custom =
            |index, code| TransactionError::InstructionError(index, InstructionError::Custom(code));

        assert_eq!(
            settlement_error(ours, &transaction, &custom(1, 16)),
            Some(SettlementError::OrderExpired)
        );
        assert_eq!(settlement_error(ours, &transaction, &custom(0, 16)), None);
        assert_eq!(settlement_error(ours, &transaction, &custom(1, 9999)), None);
        assert_eq!(
            settlement_error(ours, &transaction, &TransactionError::BlockhashNotFound),
            None
        );
    }
}
