//! The competition: the driver runs one auction through a single solver engine.
//!
//! `Competition` owns the solve flow (calling the engine) and the settle entry
//! point. It holds the concrete `infra::solver::Solver` client. One
//! `Competition` per solver engine is mounted on the API under `/{name}`.

use {
    super::{Auction, Order, auction::Id, solution::Solution},
    crate::infra::{blockchain::Solana, solver::Solver},
    itertools::Itertools,
    moka::sync::Cache,
    solana_sdk::signature::Signature,
    std::{sync::Arc, time::Duration},
    tracing::Instrument,
};

/// How long a proposed solution stays available for `settle`.
const SOLUTION_CACHE_TTL: Duration = Duration::from_secs(60);

/// Cache key for a proposed solution.
///
/// The autopilot assigns `auction_id`. The engine assigns `solution_id` in its
/// `/solve` response and may repeat ids across auctions; the driver does not
/// control how an engine numbers them. The key needs both parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    auction_id: Id,
    solution_id: u64,
}

/// A proposed solution and the auction it solves. All solutions from one
/// `solve` call share the same auction.
#[derive(Clone)]
struct CachedSolution {
    auction: Arc<Auction>,
    solution: Solution,
}

/// Orchestrates one auction through a single solver engine.
pub(crate) struct Competition {
    solver: Solver,
    blockchain: Arc<Solana>,
    /// Proposed solutions by `Key`, so `settle` can retrieve and submit the
    /// chosen one. Entries expire after `SOLUTION_CACHE_TTL`.
    solutions: Cache<Key, CachedSolution>,
}

impl Competition {
    /// Build a competition from a single solver engine and the blockchain
    /// adapter.
    pub fn new(solver: Solver, blockchain: Arc<Solana>) -> Self {
        Self {
            solver,
            blockchain,
            solutions: Cache::builder().time_to_live(SOLUTION_CACHE_TTL).build(),
        }
    }

    /// The human-readable name of the solver engine this competition uses.
    pub fn solver_name(&self) -> &str {
        self.solver.name()
    }

    /// Send the auction to the solver engine, cache its solutions, and return
    /// them.
    ///
    /// The cache stores the auction and every solution keyed by
    /// `(auction_id, solution_id)` so `settle` can later retrieve and submit
    /// the chosen solution.
    pub async fn solve(&self, auction: &Auction) -> Result<Vec<Solution>, Error> {
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

        let auction = Arc::new(auction.clone());
        for solution in &solutions {
            self.solutions.insert(
                Key {
                    auction_id: auction.id,
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

    /// Submit a previously proposed solution on chain.
    ///
    /// The work runs on a spawned task, so a client disconnect cannot cancel
    /// a settlement mid-flight. Cancellation between the consume step and
    /// the send step would destroy the solution and submit nothing.
    /// Cancellation during the send would lose the returned signature. A
    /// dropped join handle detaches the task, and the task runs to
    /// completion. The RPC client timeout bounds every step, so the task
    /// needs no abort.
    ///
    /// A successful return means the transaction reached the cluster at the
    /// RPC client's configured commitment level. This makes the Solana
    /// driver's 200 semantics match the EVM driver's: the response is only
    /// returned after the transaction is confirmed on-chain.
    pub async fn settle(
        self: &Arc<Self>,
        auction_id: Id,
        solution_id: u64,
        submission_deadline_slot: u64,
    ) -> Result<Signature, Error> {
        let this = Arc::clone(self);
        let task = tokio::spawn(
            async move {
                let result = this
                    .process_settle_request(auction_id, solution_id, submission_deadline_slot)
                    .await;
                match &result {
                    Ok(signature) => tracing::info!(%signature, "settlement submitted"),
                    Err(error) => tracing::warn!(?error, "settle failed"),
                }
                result
            }
            .instrument(tracing::Span::current()),
        );
        task.await.unwrap_or_else(|error| {
            tracing::error!(?error, "settle task panicked");
            Err(Error::TaskPanicked)
        })
    }

    /// Perform the actual on-chain settlement.
    ///
    /// This implementation performs these steps:
    /// - Look up the cached `(auction, solution)`.
    /// - Fetch the solver-provided address lookup tables and the settlement
    ///   setup accounts from RPC.
    /// - Fetch a fresh blockhash.
    /// - Build and sign a v0 settlement transaction.
    /// - Send the transaction and wait for confirmation.
    ///
    /// The method consumes the cached solution only when it hands the
    /// transaction to the network. Every failure before that point leaves
    /// the solution in place, so the caller can retry the settle. At
    /// hand-off, one atomic `Cache::remove` takes the chosen solution. A
    /// concurrent `/settle` for it then observes a missing entry and cannot
    /// settle the solution a second time.
    ///
    /// A send failure does not restore the entry. The transaction may
    /// have reached the network despite the error. A retry with a fresh
    /// blockhash could settle the solution twice.
    ///
    /// Deferred work:
    /// - admission semaphore and submission deadline slot checks,
    /// - pre-submission simulation,
    /// - ALT caching,
    /// - re-sending / retry loop.
    async fn process_settle_request(
        self: &Arc<Self>,
        auction_id: Id,
        solution_id: u64,
        _submission_deadline_slot: u64,
    ) -> Result<Signature, Error> {
        let key = Key {
            auction_id,
            solution_id,
        };
        let CachedSolution { auction, solution } = self
            .solutions
            .get(&key)
            .ok_or(Error::SolutionNotAvailable)?;

        // TODO: admission semaphore(1) and deadline slot check once we have a
        // slot stream.

        let program_id = self.blockchain.program_id();

        // The settlement carries only the orders the solution fills:
        // validation requires a trade per order, and the program settles
        // exactly the orders passed to `BeginSettle`.
        let orders = orders_with_trades(auction.orders.clone(), &solution);

        let accounts =
            super::Settlement::prepare(&self.blockchain, self.solver.pubkey(), &orders, &solution)
                .await?;

        let settlement = super::Settlement::new(
            program_id,
            auction_id,
            orders,
            solution,
            accounts.missing_buffers,
            accounts.missing_payer_atas,
        )?;

        let latest = self
            .blockchain
            .latest_confirmed_blockhash()
            .await
            .map_err(Error::Rpc)?;
        let transaction = settlement.encode(
            self.solver.keypair(),
            latest.blockhash,
            &accounts.lookup_tables,
        )?;

        // Consume the entry only now, when the transaction is about to reach
        // the network. One atomic removal takes the chosen solution. A
        // concurrent `/settle` for it then observes a missing entry and
        // cannot settle the solution again.
        if self.solutions.remove(&key).is_none() {
            return Err(Error::SolutionNotAvailable);
        }

        // TODO: bound the confirmation wait with a `tokio::timeout` once the
        // maximum settle latency policy is defined.
        let signature = self
            .blockchain
            .send_and_confirm_transaction(&transaction)
            .await
            .map_err(Error::FailedToSubmit)?;

        Ok(signature)
    }
}

/// The subset of `orders` the solution fills. The program settles exactly
/// the orders passed to `BeginSettle`. The auction's unfilled orders stay
/// out of the settlement.
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
    reason = "DeadlineExceeded and TooManyPendingSettlements are pending the deferred admission \
              semaphore and deadline slot checks"
)]
pub(crate) enum Error {
    /// The solver engine failed to produce solutions.
    #[error("solver engine failed: {0}")]
    Solver(#[from] crate::infra::solver::Error),
    /// The requested solution is not available (never solved or already
    /// settled).
    #[error("solution not available")]
    SolutionNotAvailable,
    /// The submission deadline slot has passed.
    #[error("submission deadline slot exceeded")]
    DeadlineExceeded,
    /// Too many settlements are already pending submission.
    #[error("too many pending settlements")]
    TooManyPendingSettlements,
    /// A pre-submission RPC read failed (blockhash fetch); nothing was
    /// submitted.
    #[error("rpc request failed: {0}")]
    Rpc(#[source] cow_solana_rpc::Error),
    /// The settlement transaction could not be submitted or confirmed.
    #[error("failed to submit or confirm settlement: {0}")]
    FailedToSubmit(#[source] cow_solana_rpc::Error),
    /// The settlement's on-chain accounts could not be resolved.
    #[error("failed to prepare settlement: {0}")]
    Prepare(#[from] super::settlement::PrepareError),
    /// The settlement could not be encoded.
    #[error("failed to encode settlement: {0}")]
    Settlement(#[from] super::settlement::Error),
    /// The spawned settle task panicked before it completed. The driver does
    /// not know whether the transaction reached the network.
    #[error("settle task panicked")]
    TaskPanicked,
}
