//! Manage solver competition data received by the driver through a private spi.

use anyhow::Result;
use cached::{Cached, SizedCache};
use model::solver_competition::{SolverCompetition, SolverCompetitionId};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use thiserror::Error;

/// Component used for saving and loading past solver competitions.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait SolverCompetitionStoring: Send + Sync {
    /// Saves a new solver competition entry and returns its ID.
    async fn save(&self, model: SolverCompetition) -> Result<SolverCompetitionId>;

    /// Retrieves a solver competition entry by ID.
    ///
    /// Returns a `NotFound` error if no solver competition with that ID could
    /// be found.
    async fn load(
        &self,
        id: SolverCompetitionId,
    ) -> Result<SolverCompetition, LoadSolverCompetitionError>;

    /// Retrieves the ID that will be assigned to the next solver competition
    /// entry to get saved.
    async fn next_solver_competition(&self) -> Result<SolverCompetitionId>;
}

/// Possible errors when loading a solver competition by ID.
#[derive(Debug, Error)]
pub enum LoadSolverCompetitionError {
    #[error("solver competition {0} not found")]
    NotFound(SolverCompetitionId),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// The size controls
// - how long we store competition info depending on how often the driver run loop completes
// - how much memory the cache takes up depending on how big the average response is
const CACHE_SIZE: usize = 500;

pub struct InMemoryStorage {
    last_id: AtomicU64,
    cache: Mutex<SizedCache<SolverCompetitionId, SolverCompetition>>,
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self {
            last_id: Default::default(),
            cache: Mutex::new(SizedCache::with_size(CACHE_SIZE)),
        }
    }
}

#[async_trait::async_trait]
impl SolverCompetitionStoring for InMemoryStorage {
    async fn save(&self, model: SolverCompetition) -> Result<SolverCompetitionId> {
        let id = self.last_id.fetch_add(1, Ordering::SeqCst);
        self.cache.lock().unwrap().cache_set(id, model);
        Ok(id)
    }

    async fn load(
        &self,
        id: SolverCompetitionId,
    ) -> Result<SolverCompetition, LoadSolverCompetitionError> {
        self.cache
            .lock()
            .unwrap()
            .cache_get(&id)
            .cloned()
            .ok_or(LoadSolverCompetitionError::NotFound(id))
    }

    async fn next_solver_competition(&self) -> Result<SolverCompetitionId> {
        Ok(self.last_id.load(Ordering::SeqCst))
    }
}
