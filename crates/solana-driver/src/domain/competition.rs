//! The competition: the driver runs one auction through a single solver engine.
//!
//! `Competition` owns the solve flow (calling the engine) and the settle entry
//! point. It holds the concrete `infra::solver::Solver` client. One
//! `Competition` per solver engine is mounted on the API under `/{name}`.

use {
    super::{Auction, auction::Id, solution::Solution},
    crate::infra::solver::{Error as SolverError, Solver},
    solana_sdk::pubkey::Pubkey,
};

/// Orchestrates one auction through a single solver engine.
pub struct Competition {
    solver: Solver,
}

impl Competition {
    /// Build a competition from a single solver engine.
    pub fn new(solver: Solver) -> Self {
        Self { solver }
    }

    /// The human-readable name of the solver engine this competition uses.
    pub fn solver_name(&self) -> &str {
        self.solver.name()
    }

    /// Send the auction to the solver engine and return its solutions.
    ///
    /// `program_id` is the settlement program the swap instructions are built
    /// for.
    ///
    /// If the engine fails, the driver returns `Error::Solver`.
    pub async fn solve(
        &self,
        auction: &Auction,
        program_id: Pubkey,
    ) -> Result<Vec<Solution>, Error> {
        let solutions = self.solver.solve(auction, program_id).await?;

        // TODO: store the proposed solutions in the solution cache keyed by
        // (auction_id, solution_id) once the cache lands. The cache also
        // prevents double settles and evicts old entries.

        Ok(solutions)
    }

    /// Submit a previously proposed solution on chain.
    pub fn settle(&self, _auction_id: Id, _solution_id: u64) -> Result<(), Error> {
        unimplemented!("will be implemented in follow-up PRs")
    }
}

/// An error the competition reports to the API layer.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The solver engine failed to produce solutions.
    #[error("solver engine failed: {0}")]
    Solver(#[from] SolverError),
}
