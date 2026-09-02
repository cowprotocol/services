//! One `Competition` per solver engine, mounted on the API under `/{name}`.

use {
    super::{Auction, Order, auction::Id, solution::Solution},
    crate::infra::{blockchain::Solana, solver::Solver},
    itertools::Itertools,
    moka::sync::Cache,
    solana_sdk::signature::Signature,
    std::{sync::Arc, time::Duration},
    tracing::Instrument,
};

const SOLUTION_CACHE_TTL: Duration = Duration::from_secs(60);

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

    /// The settlement worker spawned by [`settle`](Self::settle).
    ///
    /// A send failure does not restore the solution: the transaction may
    /// have reached the network despite the error, and a retry could settle
    /// twice.
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

        let orders = orders_with_trades(auction.orders.clone(), &solution);

        let settlement = super::Settlement::new(program_id, auction_id, orders, solution)?;

        let resolved = settlement
            .resolve_accounts(&self.blockchain, self.solver.pubkey())
            .await?;

        let latest = self
            .blockchain
            .latest_confirmed_blockhash()
            .await
            .map_err(Error::Rpc)?;
        let transaction = resolved.encode(self.solver.keypair(), latest.blockhash)?;

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

        // TODO: a provably unsent transaction (connect failure at send time)
        // loses the solution here; restore the cache entry on that class. Needs
        // the send/confirm split in cow-solana-rpc (planned follow-up PR).
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
    reason = "DeadlineExceeded and TooManyPendingSettlements are pending the deferred admission \
              semaphore and deadline slot checks"
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
    #[error("failed to resolve settlement accounts: {0}")]
    Resolve(#[from] super::settlement::ResolveError),
    #[error("failed to encode settlement: {0}")]
    Settlement(#[from] super::settlement::Error),
    /// The driver does not know whether the transaction reached the network.
    #[error("settle task panicked")]
    TaskPanicked,
}
