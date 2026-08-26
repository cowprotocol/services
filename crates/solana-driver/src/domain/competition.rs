//! The competition: the driver runs one auction through a single solver engine.
//!
//! `Competition` owns the solve flow (calling the engine) and the settle entry
//! point. It holds the concrete `infra::solver::Solver` client. One
//! `Competition` per solver engine is mounted on the API under `/{name}`.

use {
    super::{Auction, Order, auction::Id, solution::Solution},
    crate::infra::{blockchain::Solana, solver::Solver},
    solana_sdk::signature::Signature,
    std::{collections::HashMap, sync::Arc},
    tokio::sync::Mutex,
};

/// Orchestrates one auction through a single solver engine.
pub struct Competition {
    solver: Solver,
    blockchain: Arc<Solana>,
    /// In-memory store of proposed solutions keyed by `(auction_id,
    /// solution_id)`.
    ///
    /// This is a minimal placeholder until the real solution cache (with
    /// admission control, slot deadlines, and eviction) is introduced in a
    /// follow-up PR.
    solutions: Mutex<HashMap<(Id, u64), (Auction, Solution)>>,
}

impl Competition {
    /// Build a competition from a single solver engine and the blockchain
    /// adapter.
    pub fn new(solver: Solver, blockchain: Arc<Solana>) -> Self {
        Self {
            solver,
            blockchain,
            solutions: Mutex::new(HashMap::new()),
        }
    }

    /// Send the auction to the solver engine, cache its solutions, and return
    /// them.
    ///
    /// The cache is a minimal placeholder: it stores the auction and every
    /// solution keyed by `(auction_id, solution_id)` so `settle` can later
    /// retrieve and submit the chosen solution.
    pub async fn solve(&self, auction: &Auction) -> Result<Vec<Solution>, Error> {
        let auction_id = auction.id;
        let solutions = self
            .solver
            .solve(auction, self.blockchain.program_id())
            .await?;

        // Discard solutions with duplicate ids
        let mut by_id = HashMap::new();
        for solution in solutions {
            let id = solution.id;
            if by_id.insert(id, solution).is_some() {
                tracing::warn!(
                    solver = %self.solver.name(),
                    solution_id = id,
                    "discarding solution with duplicate id"
                );
            }
        }
        let solutions: Vec<Solution> = by_id.into_values().collect();

        {
            let mut cache = self.solutions.lock().await;
            for solution in &solutions {
                cache.insert(
                    (auction_id, solution.id),
                    (auction.clone(), solution.clone()),
                );
            }
        }

        Ok(solutions)
    }

    /// Submit a previously proposed solution on chain.
    ///
    /// This minimal implementation looks up the cached `(auction, solution)`,
    /// fetches the solver-provided address lookup tables and the settlement's
    /// setup accounts (buy-mint buffer PDAs, the solver's sell-mint ATAs) from
    /// RPC, fetches a fresh blockhash, builds and signs a v0 settlement
    /// transaction (creating the missing setup accounts), and sends it.
    ///
    /// The cached entry is only consumed once the transaction is handed to the
    /// network: every failure before that leaves the solution in place so the
    /// settle can be retried, while a send failure consumes it — the
    /// transaction may have reached the network despite the error, and
    /// re-signing with a fresh blockhash could settle twice.
    ///
    /// Production features deferred to later PRs:
    /// - admission semaphore and submission deadline slot checks,
    /// - pre-submission simulation,
    /// - ALT caching,
    /// - re-sending / retry loop.
    pub async fn settle(
        &self,
        auction_id: Id,
        solution_id: u64,
        _submission_deadline_slot: u64,
    ) -> Result<Signature, Error> {
        let (auction, solution) = self
            .solutions
            .lock()
            .await
            .get(&(auction_id, solution_id))
            .cloned()
            .ok_or(Error::SolutionNotAvailable)?;

        // TODO: admission semaphore(1) and deadline slot check once we have a
        // real cache and slot stream.

        let program_id = self.blockchain.program_id();

        // The settlement carries only the orders the solution fills:
        // validation requires a trade per order, and the program settles
        // exactly the orders passed to `BeginSettle`.
        let orders = orders_with_trades(auction.orders, &solution);

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

        let (blockhash, _last_valid_block_height) = self
            .blockchain
            .latest_blockhash()
            .await
            .map_err(Error::Rpc)?;
        let transaction =
            settlement.encode(self.solver.keypair(), blockhash, &accounts.lookup_tables)?;

        // Consume the entry only now that the transaction is about to reach
        // the network. Taking it also stops a concurrent `/settle` for the
        // same solution from double-sending: the loser observes the missing
        // entry.
        if self
            .solutions
            .lock()
            .await
            .remove(&(auction_id, solution_id))
            .is_none()
        {
            return Err(Error::SolutionNotAvailable);
        }
        let signature = self
            .blockchain
            .send_transaction(&transaction)
            .await
            .map_err(Error::FailedToSubmit)?;

        // The auction is settled: drop its remaining solutions so it cannot
        // be settled a second time through a sibling solution.
        self.solutions
            .lock()
            .await
            .retain(|(id, _), _| *id != auction_id);

        Ok(signature)
    }
}

/// The subset of `orders` the solution fills. The program settles exactly the
/// orders passed to `BeginSettle`; the auction's unfilled orders stay out of
/// the settlement.
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
pub enum Error {
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
    /// The settlement transaction could not be submitted.
    #[error("failed to submit settlement: {0}")]
    FailedToSubmit(#[source] cow_solana_rpc::Error),
    /// The settlement's on-chain accounts could not be resolved.
    #[error("failed to prepare settlement: {0}")]
    Prepare(#[from] super::settlement::PrepareError),
    /// The settlement could not be encoded.
    #[error("failed to encode settlement: {0}")]
    Settlement(#[from] super::settlement::Error),
}
