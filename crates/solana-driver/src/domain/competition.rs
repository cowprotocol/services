//! The competition: the driver runs one auction across its configured solver
//! engines.
//!
//! `Competition` owns the solve flow (concurrent fan-out to engines, reindexing
//! engine-local solution ids to driver-unique ones) and the settle seam. It
//! holds the concrete `infra::solver::Solver` clients.

use {
    super::{Auction, auction::Id, solution::Solution},
    crate::infra::solver::{Error as SolverError, Solver},
};

/// Orchestrates one auction across the configured solver engines.
pub struct Competition {
    solvers: Vec<Solver>,
}

impl Competition {
    /// Build a competition from the configured solver engines.
    pub fn new(solvers: Vec<Solver>) -> Self {
        Self { solvers }
    }

    /// Fan the auction out to every engine concurrently and collect their
    /// solutions.
    ///
    /// The driver logs and drops a failing engine. A partial failure must not
    /// kill the auction: the remaining engines still propose solutions. If
    /// every engine fails, the driver returns `Error::Solver`.
    ///
    /// The driver reindexes engine-local solution ids to driver-unique ids. The
    /// autopilot then addresses a solution by `(auction_id, solution_id)`
    /// without collisions across engines.
    pub async fn solve(&self, auction: &Auction) -> Result<Vec<Solution>, Error> {
        // TODO: add a timeout and stream results out of the fan-out (via
        // `FuturesUnordered`) while the timeout has not expired.
        let results = futures::future::join_all(self.solvers.iter().map(|solver| {
            let name = solver.name().to_owned();
            async move {
                let result = solver.solve(auction).await;
                (name, result)
            }
        }))
        .await;

        let mut solutions = Vec::new();
        let mut any_success = false;
        let mut last_error = None;
        for (name, result) in results {
            match result {
                Ok(mut engine_solutions) => {
                    any_success = true;
                    solutions.append(&mut engine_solutions);
                }
                Err(error) => {
                    tracing::warn!(solver = %name, %error, "solver engine failed");
                    last_error = Some(error);
                }
            }
        }

        // If every engine failed, return the failure. A partial failure (some
        // engines succeeded) still returns the successful solutions.
        if !any_success && let Some(error) = last_error {
            return Err(Error::Solver(error));
        }

        // Reindex engine-local solution ids to driver-unique ids. The autopilot
        // addresses solutions by (auction_id, solution_id), so ids must be
        // unique across engines within one auction.
        for (index, solution) in solutions.iter_mut().enumerate() {
            tracing::debug!(
                solver = %solution.solver,
                engine_id = solution.id,
                driver_id = index,
                "reindexing solution id"
            );
            solution.id = index as u64;
        }

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
    /// A solver engine failed to produce solutions.
    #[error("solver engine failed: {0}")]
    Solver(#[from] SolverError),
}
