//! The competition: the driver runs one auction through a single solver engine.
//!
//! `Competition` owns the solve flow (calling the engine) and the settle entry
//! point. It holds the concrete `infra::solver::Solver` client. One
//! `Competition` per solver engine is mounted on the API under `/{name}`.

use {
    super::{Auction, auction::Id, solution::Solution},
    crate::infra::{blockchain::Solana, solver::Solver},
    itertools::Itertools,
    moka::sync::Cache,
    std::{sync::Arc, time::Duration},
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
#[expect(dead_code, reason = "`settle` does not read the fields yet")]
#[derive(Clone)]
struct CachedSolution {
    auction: Arc<Auction>,
    solution: Solution,
}

/// Orchestrates one auction through a single solver engine.
pub struct Competition {
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
        let solutions = Cache::builder().time_to_live(SOLUTION_CACHE_TTL).build();
        Self {
            solver,
            blockchain,
            solutions,
        }
    }

    /// The human-readable name of the solver engine this competition uses.
    pub fn solver_name(&self) -> &str {
        self.solver.name()
    }

    /// Send the auction to the solver engine, cache its solutions, and return
    /// them.
    ///
    /// The cache stores the auction and every solution keyed by `(auction_id,
    /// solution_id)` so `settle` can later retrieve and submit the chosen
    /// solution.
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
    // TODO: when `settle` accepts a request, take the cache entry with
    // `Cache::remove`. This prevents a double settle of the same solution.
    pub fn settle(&self, _auction_id: Id, _solution_id: u64) -> Result<(), Error> {
        unimplemented!("will be implemented in follow-up PRs")
    }
}

/// An error the competition reports to the API layer.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The solver engine failed to produce solutions.
    #[error("solver engine failed: {0}")]
    Solver(#[from] crate::infra::solver::Error),
}
