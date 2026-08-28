//! The competition: the driver runs one auction through a single solver engine.
//!
//! `Competition` owns the solve flow (calling the engine) and the settle entry
//! point. It holds the concrete `infra::solver::Solver` client. One
//! `Competition` per solver engine is mounted on the API under `/{name}`.

use {
    super::{Auction, auction::Id, solution::Solution},
    crate::infra::{blockchain::Solana, solver::Solver},
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
    /// This is a minimal, temporary cache. It does not enforce admission
    /// limits, slot deadlines, or eviction.
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
        let auction_id = auction.id;
        let solutions = self
            .solver
            .solve(auction, self.blockchain.program_id())
            .await?;

        // Discard solutions with duplicate ids.
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
    pub fn settle(&self, _auction_id: Id, _solution_id: u64) -> Result<(), Error> {
        unimplemented!("will be implemented in follow-up PRs")
    }
}

/// An error the competition reports to the API layer.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
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
    /// The settlement transaction could not be submitted or confirmed.
    #[error("failed to submit or confirm settlement: {0}")]
    FailedToSubmit(#[source] cow_solana_rpc::Error),
    /// The settlement could not be encoded.
    #[error("failed to encode settlement: {0}")]
    Settlement(#[from] super::settlement::Error),
    /// The spawned settle task panicked before it completed. The driver does
    /// not know whether the transaction reached the network.
    #[error("settle task panicked")]
    TaskPanicked,
}
