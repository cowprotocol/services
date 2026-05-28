//! Shared cache fronting every configured banned-user backend. Stores one
//! entry per address (not per address × backend) since callers only ask
//! "banned by anyone?"; backends stay as pure fetchers.

use {
    alloy_primitives::Address,
    async_trait::async_trait,
    futures::{StreamExt, future::join_all, stream},
    moka::sync::Cache,
    std::{
        collections::HashSet,
        sync::{Arc, Weak},
        time::{Duration, Instant},
    },
};

/// Caps in-flight fetches so a large miss batch can't burst the backends.
const MAX_CONCURRENT_LOOKUPS: usize = 10;
const CACHE_EXPIRY: Duration = Duration::from_secs(60 * 60);
const MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
struct Entry {
    is_banned: bool,
    last_updated: Instant,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum BackendError {
    #[error("chainalysis lookup failed")]
    Chainalysis(#[from] alloy_contract::Error),

    #[error("hermod lookup failed")]
    Hermod(#[from] super::hermod::Error),
}

/// Pure banned-address fetcher; caching and refresh live in [`Cached`].
#[async_trait]
pub(super) trait Backend: Send + Sync + 'static {
    async fn fetch(&self, address: Address) -> Result<bool, BackendError>;

    fn name(&self) -> &'static str;
}

/// Single cache fronting every configured backend. A miss fans out to every
/// backend in parallel and stores the OR of the results.
pub(super) struct Cached {
    backends: Vec<Box<dyn Backend>>,
    cache: Cache<Address, Entry>,
}

impl Cached {
    /// Returns `None` when no backends are configured.
    pub(super) fn new(backends: Vec<Box<dyn Backend>>, max_capacity: u64) -> Option<Arc<Self>> {
        if backends.is_empty() {
            return None;
        }
        let cached = Arc::new(Self {
            backends,
            cache: Cache::builder().max_capacity(max_capacity).build(),
        });
        cached.spawn_maintenance_task();
        Some(cached)
    }

    /// Returns the subset reported as banned by any backend. Misses fan out
    /// to backends concurrently.
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
        for (address, is_banned) in fetched {
            let Some(is_banned) = is_banned else { continue };
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

    /// `Some(true)` as soon as any backend confirms a ban — a failure
    /// elsewhere must not mask a positive hit. `None` means no confirmation
    /// and at least one failure, so the caller skips caching.
    async fn fetch_all(&self, address: Address) -> Option<bool> {
        let results = join_all(self.backends.iter().map(|b| fetch_one(b.as_ref(), address))).await;
        if results.iter().any(|r| matches!(r, Some(true))) {
            Some(true)
        } else if results.iter().any(Option::is_none) {
            None
        } else {
            Some(false)
        }
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

    /// `None` (existing entry preserved) when `fetch_all` is uncertain — no
    /// positive confirmation and at least one backend failed.
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
    /// entries so callers rarely observe a cold miss. Holds a [`Weak`] handle
    /// so the task exits once the last external [`Arc`] is dropped.
    fn spawn_maintenance_task(self: &Arc<Self>) {
        let weak: Weak<Self> = Arc::downgrade(self);
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(MAINTENANCE_TIMEOUT);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let Some(this) = weak.upgrade() else { return };
                let now = Instant::now();
                let expired = this.expired(now);

                let refreshed: Vec<_> = stream::iter(expired)
                    .map(|address| this.refresh(*address))
                    .buffer_unordered(MAX_CONCURRENT_LOOKUPS)
                    .collect()
                    .await;

                for (address, entry) in refreshed.into_iter().flatten() {
                    this.cache.insert(address, entry);
                }
            }
        });
    }
}

/// Logs and swallows backend errors so callers can OR successful results.
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
