//! Process-shared cache of fast-path quote solutions.
//!
//! A fast-path quote runs against a throwaway auction whose order is synthetic
//! and unsigned, so its solution cannot be encoded into a submittable
//! settlement until the real order exists at settle time; `/settle_fast_path`
//! re-encodes it against that order. The solution is cached here at quote time.
//!
//! The cache is shared across every solver config on the driver (keyed by the
//! globally-unique quote id), not held per `Competition`. A team may run its
//! quoter and solver as separate configs, so a quote cached by one config must
//! be settleable by another.

use {
    super::{Auction, Solution},
    crate::domain::quote,
    moka::future::Cache,
    std::time::Duration,
};

/// Upper bound on cached fast-path quote solutions across all solver configs on
/// this driver. Each fast-path quote pushes one entry.
const MAX_CACHED_QUOTE_SOLUTIONS: usize = 100;

/// How long a cached fast-path quote solution stays settleable.
const QUOTE_SOLUTION_TTL: Duration = Duration::from_secs(30);

/// A quote solution plus the single-order auction it was solved in (kept for
/// its token set, which the re-encode needs).
#[derive(Debug, Clone)]
pub struct CachedQuoteSolution {
    pub auction: Auction,
    pub solution: Solution,
}

/// Fast-path quote solutions, written by `/quote` and consumed by
/// `/settle_fast_path`. Cloning shares one underlying store (moka is
/// `Arc`-backed), so every solver config on the driver reads and writes the
/// same cache.
#[derive(Debug, Clone)]
pub struct FastPathQuoteCache(Cache<quote::Id, CachedQuoteSolution>);

impl FastPathQuoteCache {
    pub fn new() -> Self {
        Self(
            Cache::builder()
                .max_capacity(MAX_CACHED_QUOTE_SOLUTIONS as u64)
                .time_to_live(QUOTE_SOLUTION_TTL)
                .build(),
        )
    }

    /// Cache `solution` under `quote_id` for a later fast-path settle.
    pub async fn store(&self, quote_id: quote::Id, auction: Auction, solution: Solution) {
        self.0
            .insert(quote_id, CachedQuoteSolution { auction, solution })
            .await;
    }

    /// Remove and return the solution cached under `quote_id`, if any. Consumed
    /// on read: a fast-path solution settles at most once.
    pub async fn take(&self, quote_id: &quote::Id) -> Option<CachedQuoteSolution> {
        self.0.remove(quote_id).await
    }
}

impl Default for FastPathQuoteCache {
    fn default() -> Self {
        Self::new()
    }
}
