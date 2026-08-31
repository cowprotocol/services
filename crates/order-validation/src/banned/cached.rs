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
    /// The lookup failed and `is_banned` is a fail-open placeholder until
    /// a maintenance-task retry succeeds.
    uncertain: bool,
}

impl Entry {
    /// Creates a new [`Entry`] with `last_updated` set to [`Instant::now`]-
    fn new(is_banned: bool) -> Self {
        Self {
            is_banned,
            last_updated: Instant::now(),
            uncertain: false,
        }
    }

    /// Creates an [`Entry`] for a failed lookup.
    fn uncertain() -> Self {
        Self {
            is_banned: false,
            last_updated: Instant::now(),
            uncertain: true,
        }
    }
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

        for (address, is_banned) in fetched {
            let entry = match is_banned {
                Some(is_banned) => Entry::new(is_banned),
                None => Entry::uncertain(),
            };
            if entry.is_banned {
                banned.insert(address);
            }
            self.cache.insert(address, entry);
        }

        banned
    }

    /// `Some(true)` as soon as any backend confirms a ban, since a failure
    /// elsewhere must not mask a positive hit. `None` means no confirmation
    /// and at least one failure.
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
    /// tick may miss the window, plus uncertain entries awaiting a retry.
    fn expired(&self, now: Instant) -> Vec<Arc<Address>> {
        self.cache
            .iter()
            .filter_map(|(address, entry)| {
                let due = entry.uncertain
                    || now
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
        Some((address, Entry::new(is_banned)))
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    struct FlakyBackend {
        calls: Arc<AtomicUsize>,
        fail: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Backend for FlakyBackend {
        async fn fetch(&self, _: Address) -> Result<bool, BackendError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                Err(BackendError::Hermod(
                    super::super::hermod::Error::UnexpectedStatus(
                        reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                    ),
                ))
            } else {
                Ok(false)
            }
        }

        fn name(&self) -> &'static str {
            "flaky"
        }
    }

    fn setup(fail: bool) -> (Arc<Cached>, Arc<AtomicUsize>, Arc<AtomicBool>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let failing = Arc::new(AtomicBool::new(fail));
        let backend = FlakyBackend {
            calls: calls.clone(),
            fail: failing.clone(),
        };
        let cached = Cached::new(vec![Box::new(backend)], 100).unwrap();
        (cached, calls, failing)
    }

    #[tokio::test]
    async fn failed_lookup_is_cached_not_refetched_inline() {
        let (cached, calls, _failing) = setup(true);
        let addresses = HashSet::from([Address::repeat_byte(1)]);

        assert!(cached.check(&addresses).await.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        assert!(cached.check(&addresses).await.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn uncertain_entries_are_due_for_maintenance_and_recover() {
        let (cached, _calls, failing) = setup(true);
        let address = Address::repeat_byte(1);
        let addresses = HashSet::from([address]);

        assert!(cached.check(&addresses).await.is_empty());
        assert_eq!(cached.expired(Instant::now()).len(), 1);

        failing.store(false, Ordering::SeqCst);
        let (address, entry) = cached.refresh(address).await.unwrap();
        cached.cache.insert(address, entry);

        assert!(cached.expired(Instant::now()).is_empty());
    }
}
