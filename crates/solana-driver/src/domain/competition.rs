//! One `Competition` per solver engine, mounted on the API under `/{name}`.

use {
    super::{
        Auction,
        Order,
        auction::Id,
        settlement::{ResolvedSettlement, is_push_shortfall},
        solution::Solution,
    },
    crate::infra::{blockchain::Solana, solver::Solver},
    itertools::Itertools,
    moka::sync::Cache,
    solana_sdk::{
        hash::Hash,
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
    /// Ascending.
    push_reduction_bps: Vec<u16>,
}

impl Competition {
    pub fn new(solver: Solver, blockchain: Arc<Solana>, push_reduction_bps: Vec<u16>) -> Self {
        Self {
            solver,
            blockchain,
            solutions: Cache::builder().time_to_live(SOLUTION_CACHE_TTL).build(),
            push_reduction_bps,
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
        let transaction = self
            .simulate_candidates(&resolved, latest.blockhash, cu_estimate)
            .await?;

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
        .map_err(Error::FailedToSubmit)?;

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

    /// Simulate the promise and its reduced-push candidates concurrently and
    /// return the passing transaction with the smallest reduction.
    ///
    /// A push the buffer cannot cover is the only failure a smaller push can
    /// fix (the amount feeds nothing but the `FinalizeSettle` transfer and a
    /// limit-price check it can only tighten), so a candidate stands in for
    /// the promise on exactly that failure.
    ///
    /// The ladder is fixed because a failed simulation hides the buffer's
    /// post-swap balance; a sequential probe at the limit-price floor with the
    /// buffer PDA in the request's `accounts` would measure it instead.
    async fn simulate_candidates(
        &self,
        resolved: &ResolvedSettlement,
        blockhash: Hash,
        cu_estimate: Option<u32>,
    ) -> Result<VersionedTransaction, Error> {
        let candidates = self.candidates(resolved, blockhash)?;
        observe_transaction(&candidates[0].transaction, cu_estimate);

        tracing::debug!(
            candidates = candidates.len(),
            "simulating settlement candidates"
        );
        let simulations = futures::future::join_all(
            candidates
                .iter()
                .map(|candidate| self.blockchain.simulate_transaction(&candidate.transaction)),
        )
        .await;
        let mut outcomes = candidates.into_iter().zip(simulations);

        // Without the promise's result there is no evidence a reduction was
        // needed.
        let (promise, simulation) = outcomes
            .next()
            .expect("the promise is always the first candidate");
        let simulation = simulation.map_err(Error::Rpc)?;
        let Some(err) = simulation.err else {
            tracing::debug!("settlement simulation passed");
            self.count_push_reduction("unneeded", 0);
            return Ok(promise.transaction);
        };
        let err = TransactionError::from(err);
        if !is_push_shortfall(&promise.transaction, &err) {
            tracing::warn!(?err, logs = ?simulation.logs, "settlement simulation failed");
            return Err(Error::SimulationFailed(err.into()));
        }

        let mut failures = Vec::new();
        for (candidate, simulation) in outcomes {
            match simulation {
                Ok(simulation) if simulation.err.is_none() => {
                    tracing::info!(
                        reduction_bps = candidate.reduction_bps,
                        promised = ?promise.pushes,
                        pushed = ?candidate.pushes,
                        "settling with reduced pushes"
                    );
                    self.count_push_reduction("recovered", candidate.reduction_bps);
                    return Ok(candidate.transaction);
                }
                simulation => failures.push((
                    candidate.reduction_bps,
                    simulation.map(|simulation| simulation.err),
                )),
            }
        }
        tracing::warn!(
            ?err,
            logs = ?simulation.logs,
            ?failures,
            "settlement simulation failed on a push shortfall no reduction covered"
        );
        self.count_push_reduction("exhausted", 0);
        Err(Error::SimulationFailed(err.into()))
    }

    fn candidates(
        &self,
        resolved: &ResolvedSettlement,
        blockhash: Hash,
    ) -> Result<Vec<Candidate>, Error> {
        let keypair = self.solver.keypair();
        let mut candidates = vec![Candidate {
            reduction_bps: 0,
            pushes: resolved.pushes()?,
            transaction: resolved.encode(keypair, blockhash)?,
        }];
        for &reduction_bps in &self.push_reduction_bps {
            let reduced = resolved.reduced(reduction_bps)?;
            let pushes = reduced.pushes()?;
            // Equal pushes would only repeat the previous simulation.
            if candidates.last().is_some_and(|last| last.pushes == pushes) {
                continue;
            }
            candidates.push(Candidate {
                reduction_bps,
                pushes,
                transaction: reduced.encode(keypair, blockhash)?,
            });
        }
        Ok(candidates)
    }

    fn count_push_reduction(&self, outcome: &str, bps: u16) {
        metrics()
            .push_reductions
            .with_label_values(&[outcome, &bps.to_string(), self.solver.name()])
            .inc();
    }
}

/// The promise (`reduction_bps == 0`) or a reduced-push variant of it.
struct Candidate {
    reduction_bps: u16,
    pushes: Vec<u64>,
    transaction: VersionedTransaction,
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
    #[error("failed to submit or confirm settlement: {0}")]
    FailedToSubmit(#[source] cow_solana_rpc::Error),
    #[error("failed to submit or confirm an order creation: {0}")]
    FailedToCreate(#[source] cow_solana_rpc::Error),
    /// The pre-submission simulation failed. The transaction was not sent.
    #[error("settlement simulation failed: {0}")]
    SimulationFailed(#[from] cow_solana_rpc::UiTransactionError),
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
    /// Simulated settlements by outcome (`unneeded`, `recovered`,
    /// `exhausted`), reduction applied in basis points, and solver.
    #[metric(labels("outcome", "bps", "solver"))]
    push_reductions: prometheus::IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}

/// Record the built transaction's footprint against the per-transaction bytes,
/// account, and compute-unit ceilings.
fn observe_transaction(transaction: &VersionedTransaction, cu_estimate: Option<u32>) {
    let metrics = metrics();
    if let Ok(bytes) = bincode::serialized_size(transaction) {
        metrics.transaction_bytes.observe(bytes as f64);
    }
    metrics
        .transaction_accounts
        .observe(account_count(transaction) as f64);
    if let Some(cu) = cu_estimate {
        metrics.compute_units.observe(f64::from(cu));
    }
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
        Error::FailedToSubmit(_) => "submit_failed",
        Error::FailedToCreate(_) => "creation_failed",
        Error::SimulationFailed(_) => "simulation_failed",
        Error::Resolve(_) => "resolve_failed",
        Error::Settlement(_) => "invalid_settlement",
        Error::TaskPanicked => "panicked",
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            domain::{Settlement, Side, Trade, order_uid::OrderUid},
            infra::config,
        },
        cow_settlement_interface::{data::intent::OrderIntent, pda::order::find_order_pda},
        cow_solana_rpc::{MocksMap, RpcRequest, SolanaRPC},
        solana_sdk::pubkey::Pubkey,
        solana_testlib::temp_keypair,
        std::collections::HashMap,
    };

    /// [CreateBuffers, CreateAta, BeginSettle, FinalizeSettle]: the mock
    /// reports every account missing and there is no compute-unit estimate.
    const FINALIZE_INDEX: u8 = 3;

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn order(program_id: &Pubkey) -> Order {
        let mut order = Order {
            uid: OrderUid([0; 32]),
            owner: pubkey(0x22),
            sell_token: pubkey(0x33),
            buy_token: pubkey(0x44),
            sell_token_account: pubkey(0x55),
            buy_token_account: pubkey(0x66),
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: u32::MAX,
            side: Side::Sell,
            partially_fillable: false,
            order_pda: Pubkey::default(),
            app_data: [0; 32],
        };
        let uid = OrderIntent::from(&order).uid();
        order.uid = OrderUid(uid.to_bytes());
        order.order_pda = find_order_pda(program_id, &uid).0;
        order
    }

    fn solution(order: &Order, executed_buy: u64) -> Solution {
        Solution {
            id: 0,
            solver: pubkey(0x99),
            prices: HashMap::new(),
            trades: vec![Trade {
                order_uid: order.uid,
                executed_sell: 1_000,
                executed_buy,
            }],
            interactions: Vec::new(),
            address_lookup_tables: Vec::new(),
            cu_estimate: None,
        }
    }

    fn failed_simulation(err: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "context": { "slot": 1, "apiVersion": "2.0.0" },
            "value": {
                "err": err,
                "logs": [],
                "accounts": null,
                "unitsConsumed": 0,
                "returnData": null,
            }
        })
    }

    fn failed_push() -> serde_json::Value {
        failed_simulation(serde_json::json!({
            "InstructionError": [FINALIZE_INDEX, { "Custom": 1 }]
        }))
    }

    /// Answers `simulateTransaction` with `simulations` in order, then with
    /// the mock default: success.
    fn competition(
        push_reduction_bps: Vec<u16>,
        simulations: Vec<serde_json::Value>,
    ) -> Competition {
        let keypair_file = temp_keypair();
        let solver = Solver::new(&config::Solver {
            name: "mock".to_owned(),
            endpoint: "http://127.0.0.1:1".parse().unwrap(),
            signer_keypair: keypair_file.path().to_path_buf(),
            solve_every_nth_auction: None,
        })
        .unwrap();
        let mut mocks = MocksMap::default();
        for simulation in simulations {
            mocks.insert(RpcRequest::SimulateTransaction, simulation);
        }
        let blockchain = Arc::new(Solana::new(
            SolanaRPC::new_mock_with_mocks_map(mocks),
            pubkey(0xaa),
        ));
        Competition::new(solver, blockchain, push_reduction_bps)
    }

    async fn resolved(competition: &Competition, executed_buy: u64) -> ResolvedSettlement {
        let program_id = competition.blockchain.program_id();
        let order = order(&program_id);
        let settlement = Settlement::new(
            program_id,
            Id::new(7).unwrap(),
            vec![order.clone()],
            solution(&order, executed_buy),
        )
        .unwrap();
        settlement
            .resolve_accounts(&competition.blockchain, competition.solver.pubkey())
            .await
            .unwrap()
    }

    /// The 1 bps step rounds to the promise's pushes; simulated instead of
    /// skipped, it would absorb the second failure and hand the settlement
    /// to 50 bps.
    #[tokio::test]
    async fn settles_the_smallest_passing_reduction() {
        let competition = competition(vec![1, 50, 100, 200], vec![failed_push(), failed_push()]);
        let resolved = resolved(&competition, 2_100).await;

        let transaction = competition
            .simulate_candidates(&resolved, Hash::default(), None)
            .await
            .unwrap();

        let reduced = resolved.reduced(100).unwrap();
        assert_eq!(reduced.pushes().unwrap(), vec![2_079]);
        assert_eq!(
            transaction,
            reduced
                .encode(competition.solver.keypair(), Hash::default())
                .unwrap()
        );
    }

    #[tokio::test]
    async fn fails_closed_when_the_promise_fails_without_a_shortfall() {
        let competition = competition(
            vec![50, 100, 200],
            vec![failed_simulation(serde_json::json!("AccountInUse"))],
        );
        let resolved = resolved(&competition, 2_100).await;

        let error = competition
            .simulate_candidates(&resolved, Hash::default(), None)
            .await
            .unwrap_err();

        assert!(matches!(error, Error::SimulationFailed(_)), "{error:?}");
    }
}
