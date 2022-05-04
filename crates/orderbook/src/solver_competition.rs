//! Manage solver competition data received by the driver through a private spi.

use cached::{Cached, SizedCache};
use model::solver_competition::SolverCompetitionResponse;
use std::sync::Mutex;

type AuctionId = u64;

const CACHE_SIZE: usize = 1000;

pub struct SolverCompetition {
    cache: Mutex<SizedCache<AuctionId, SolverCompetitionResponse>>,
}

impl Default for SolverCompetition {
    fn default() -> Self {
        Self {
            cache: Mutex::new(SizedCache::with_size(CACHE_SIZE)),
        }
    }
}

impl SolverCompetition {
    pub fn get(&self, auction_id: AuctionId) -> Option<SolverCompetitionResponse> {
        self.cache.lock().unwrap().cache_get(&auction_id).cloned()
    }

    pub fn set(&self, auction_id: AuctionId, model: SolverCompetitionResponse) {
        self.cache.lock().unwrap().cache_set(auction_id, model);
    }
}
