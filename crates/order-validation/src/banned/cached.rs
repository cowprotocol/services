//! Shared cache that sits in front of every configured remote banned-user
//! backend.
//!
//! `Users::banned` answers a single question — "is this address banned by any
//! of our sources?" — so the cache stores one entry per address rather than
//! one per (address, backend). Backends remain pure fetchers; this module is
//! the only layer that knows about caching, batching, or background refresh.

use {
    alloy_primitives::Address,
    async_trait::async_trait,
    futures::{StreamExt, future::join_all, stream},
    moka::sync::Cache,
    std::{
        collections::HashSet,
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// Caps the number of in-flight per-address fetches so a large batch of cache
/// misses (or a large maintenance refresh) does not burst the backends.
const MAX_CONCURRENT_LOOKUPS: usize = 10;
const CACHE_EXPIRY: Duration = Duration::from_secs(60 * 60);
const MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
struct Entry {
    is_banned: bool,
    last_updated: Instant,
}

/// Union of every backend's native error type. `#[from]` lets each impl
/// `?`-propagate its own error into this enum without manual conversion.
#[derive(Debug, thiserror::Error)]
pub(super) enum BackendError {
    #[error("chainalysis lookup failed")]
    Chainalysis(#[from] alloy_contract::Error),

    #[error("hermod lookup failed")]
    Hermod(#[from] super::hermod::HermodError),
}

/// Pure banned-address fetcher. Implementations only need to know how to ask
/// their underlying source — caching, batching, and refresh are handled by
/// the surrounding [`Cached`] layer.
#[async_trait]
pub(super) trait Backend: Send + Sync + 'static {
    async fn fetch(&self, address: Address) -> Result<bool, BackendError>;

    /// Short identifier used as a log tag to distinguish failures across
    /// backends.
    fn name(&self) -> &'static str;
}

/// Single cache fronting every configured backend. A miss fans out to every
/// backend in parallel and stores the OR of the results.
pub(super) struct Cached {
    backends: Vec<Box<dyn Backend>>,
    cache: Cache<Address, Entry>,
}

impl Cached {
    /// Returns `None` if no backends are configured — caching makes no sense
    /// without something to cache.
    pub(super) fn new(backends: Vec<Box<dyn Backend>>, max_capacity: u64) -> Option<Arc<Self>> {
        if backends.is_empty() {
            return None;
        }
        let cached = Arc::new(Self {
            backends,
            cache: Cache::builder().max_capacity(max_capacity).build(),
        });
        cached.clone().spawn_maintenance_task();
        Some(cached)
    }

    /// Returns the subset of `addresses` that any configured backend reports
    /// as banned. Cache hits are served immediately; misses are fetched
    /// concurrently and written back.
    pub(super) async fn check(&self, addresses: &HashSet<Address>) -> HashSet<Address> {
        let mut banned = HashSet::new();
        let mut need_lookup = Vec::new();
        for address in addresses {
            match self.cache.get(address) {
                Some(entry) => {
                    entry.is_banned.then(|| banned.insert(*address));
                }
                None => need_lookup.push(*address),
            }
        }

        let fetched: Vec<_> = stream::iter(need_lookup)
            .map(|address| async move { (address, self.fetch_all(address).await) })
            .buffer_unordered(MAX_CONCURRENT_LOOKUPS)
            .collect()
            .await;

        let now = Instant::now();
        for (address, is_banned) in fetched.into_iter().flat_map(|(a, r)| r.map(|b| (a, b))) {
            self.cache.insert(
                address,
                Entry {
                    is_banned,
                    last_updated: now,
                },
            );
            if is_banned {
                banned.insert(address);
            }
        }

        banned
    }

    /// Queries every configured backend for this address in parallel. Returns
    /// `Some(is_banned)` only if every backend reported successfully; if any
    /// failed, returns `None` so a partial result doesn't get cached and
    /// mask a hit from the failing source.
    async fn fetch_all(&self, address: Address) -> Option<bool> {
        join_all(self.backends.iter().map(|b| fetch_one(b.as_ref(), address)))
            .await
            .into_iter()
            .collect::<Option<Vec<bool>>>()
            .map(|results| results.into_iter().any(|banned| banned))
    }

    /// Collects cache entries close enough to expiry that the next maintenance
    /// tick may miss the window.
    fn expired(&self, now: Instant) -> Vec<Arc<Address>> {
        self.cache
            .iter()
            .filter_map(|(address, entry)| {
                let due = now
                    .checked_duration_since(entry.last_updated)
                    .unwrap_or_default()
                    >= CACHE_EXPIRY - MAINTENANCE_TIMEOUT;
                due.then_some(address)
            })
            .collect()
    }

    /// Re-queries every backend for an address. Returns `None` (and leaves
    /// the existing entry alone) when any configured backend fails, so a
    /// transient outage doesn't poison the cache.
    async fn refresh(&self, address: Address) -> Option<(Address, Entry)> {
        let is_banned = self.fetch_all(address).await?;
        Some((
            address,
            Entry {
                is_banned,
                last_updated: Instant::now(),
            },
        ))
    }

    /// Spawns a background task that periodically refreshes near-expiry cache
    /// entries so callers rarely observe a cold miss.
    fn spawn_maintenance_task(self: Arc<Self>) {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(MAINTENANCE_TIMEOUT);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let now = Instant::now();
                let expired = self.expired(now);

                let refreshed: Vec<_> = stream::iter(expired)
                    .map(|address| self.refresh(*address))
                    .buffer_unordered(MAX_CONCURRENT_LOOKUPS)
                    .collect()
                    .await;

                for (address, entry) in refreshed.into_iter().flatten() {
                    self.cache.insert(address, entry);
                }
            }
        });
    }
}

/// Calls one backend and logs (but swallows) the error so the caller can OR
/// successful results together without dealing with per-backend error types.
async fn fetch_one(backend: &dyn Backend, address: Address) -> Option<bool> {
    match backend.fetch(address).await {
        Ok(banned) => Some(banned),
        Err(err) => {
            tracing::warn!(
                backend = backend.name(),
                ?address,
                ?err,
                "failed to fetch banned status",
            );
            None
        }
    }
}
