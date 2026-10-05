//! Per-account cache of fast-path quote solutions.
//!
//! A fast-path quote is solved at quote time but can only be settled once the
//! real signed order exists, so the solution is cached here and re-encoded
//! against that order at settle time. Each solver account gets its own cache: a
//! team may run its quoter and solver as separate configs that share an
//! account, so one settles what the other cached, while different accounts stay
//! isolated.

use {
    super::{Auction, Solution},
    crate::domain::quote,
    eth_domain_types as eth,
    moka::future::Cache,
    std::{collections::HashMap, sync::Arc, time::Duration},
};

/// Max cached fast-path quote solutions per solver account.
const MAX_CACHED_QUOTE_SOLUTIONS: usize = 100;

/// How long a cached fast-path quote solution stays settleable.
const QUOTE_SOLUTION_TTL: Duration = Duration::from_secs(30);

/// A quote solution and the single-order auction it was solved in.
#[derive(Debug, Clone)]
pub struct CachedQuoteSolution {
    pub auction: Auction,
    pub solution: Solution,
}

/// One bounded cache of fast-path quote solutions per solver account, so no
/// account can evict another's. Cloning shares the same caches.
#[derive(Debug, Clone)]
pub struct FastPathQuoteCache(Arc<HashMap<eth::Address, Cache<quote::Id, CachedQuoteSolution>>>);

impl FastPathQuoteCache {
    /// Builds one bounded cache per distinct solver account.
    pub fn new(accounts: impl IntoIterator<Item = eth::Address>) -> Self {
        let mut caches = HashMap::new();
        for account in accounts {
            caches.entry(account).or_insert_with(|| {
                Cache::builder()
                    .max_capacity(MAX_CACHED_QUOTE_SOLUTIONS as u64)
                    .time_to_live(QUOTE_SOLUTION_TTL)
                    .build()
            });
        }
        Self(Arc::new(caches))
    }

    /// Cache `solution` under `account` for a later fast-path settle.
    pub async fn store(
        &self,
        account: eth::Address,
        quote_id: quote::Id,
        auction: Auction,
        solution: Solution,
    ) {
        if let Some(cache) = self.0.get(&account) {
            cache
                .insert(quote_id, CachedQuoteSolution { auction, solution })
                .await;
        }
    }

    /// Remove and return the solution cached under `account`, if any. Consumed
    /// on read: a fast-path solution settles at most once.
    pub async fn take(
        &self,
        account: eth::Address,
        quote_id: &quote::Id,
    ) -> Option<CachedQuoteSolution> {
        self.0.get(&account)?.remove(quote_id).await
    }
}
