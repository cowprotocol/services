//! Shared cache fronting every configured banned-user backend. Stores one
//! entry per address (not per address × backend) since callers only ask
//! "banned by anyone?"; backends stay as pure fetchers.

use {
    super::{CacheExpiry, metrics::Metrics},
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
const MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, PartialEq)]
enum Verdict {
    Banned,
    NotBanned,
    /// Every lookup failed. Treated as not banned until a maintenance-task
    /// retry succeeds.
    Unknown,
}

#[derive(Clone)]
struct Entry {
    verdict: Verdict,
    last_updated: Instant,
}

impl Entry {
    /// Creates a new [`Entry`] with `last_updated` set to [`Instant::now`]-
    fn new(verdict: Verdict) -> Self {
        Self {
            verdict,
            last_updated: Instant::now(),
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
    /// Entry ages at which the maintenance task refreshes them, one tick
    /// before the configured expiry. Zero refreshes every tick.
    refresh_after: CacheExpiry,
}

impl Cached {
    /// Returns `None` when no backends are configured.
    pub(super) fn new(
        backends: Vec<Box<dyn Backend>>,
        max_capacity: u64,
        expiry: CacheExpiry,
    ) -> Option<Arc<Self>> {
        if backends.is_empty() {
            return None;
        }
        let cached = Arc::new(Self {
            backends,
            cache: Cache::builder().max_capacity(max_capacity).build(),
            refresh_after: CacheExpiry {
                not_banned: expiry.not_banned.saturating_sub(MAINTENANCE_TIMEOUT),
                banned: expiry.banned.saturating_sub(MAINTENANCE_TIMEOUT),
            },
        });
        Metrics::currently_banned(0);
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
                    (entry.verdict == Verdict::Banned).then(|| banned.insert(*address));
                }
                None => need_lookup.push(*address),
            }
        }
        Metrics::cache_hits(addresses.len() - need_lookup.len(), need_lookup.len());

        let fetched: Vec<_> = stream::iter(need_lookup)
            .map(|address| async move { (address, self.fetch_all(address).await) })
            .buffer_unordered(MAX_CONCURRENT_LOOKUPS)
            .collect()
            .await;

        for (address, is_banned) in fetched {
            let verdict = match is_banned {
                Some(true) => Verdict::Banned,
                Some(false) => Verdict::NotBanned,
                None => Verdict::Unknown,
            };
            if verdict == Verdict::Banned {
                banned.insert(address);
            }
            self.store(address, verdict);
        }

        banned
    }

    /// Caches `verdict`, reporting a ban the cache did not already know about.
    /// Returns how the number of banned entries changed.
    fn store(&self, address: Address, verdict: Verdict) -> i64 {
        let known_banned = self
            .cache
            .get(&address)
            .is_some_and(|entry| entry.verdict == Verdict::Banned);
        let banned = verdict == Verdict::Banned;
        if banned && !known_banned {
            Metrics::detected();
        }
        self.cache.insert(address, Entry::new(verdict));
        i64::from(banned) - i64::from(known_banned)
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

    /// Walks the cache once, returning the entries due for a refresh and how
    /// many entries are currently banned. Due are the entries close enough to
    /// expiry that the next maintenance tick may miss the window, plus
    /// [`Verdict::Unknown`] entries awaiting a retry.
    fn scan(&self, now: Instant) -> (Vec<Arc<Address>>, i64) {
        let mut due = Vec::new();
        let mut banned = 0;
        for (address, entry) in self.cache.iter() {
            if entry.verdict == Verdict::Banned {
                banned += 1;
            }
            let refresh_after = match entry.verdict {
                Verdict::Banned => self.refresh_after.banned,
                Verdict::NotBanned => self.refresh_after.not_banned,
                Verdict::Unknown => Duration::ZERO,
            };
            let age = now
                .checked_duration_since(entry.last_updated)
                .unwrap_or_default();
            if age >= refresh_after {
                due.push(address);
            }
        }
        (due, banned)
    }

    /// `None` (existing entry preserved) when `fetch_all` is uncertain — no
    /// positive confirmation and at least one backend failed.
    async fn refresh(&self, address: Address) -> Option<(Address, Verdict)> {
        let is_banned = self.fetch_all(address).await?;
        let verdict = if is_banned {
            Verdict::Banned
        } else {
            Verdict::NotBanned
        };
        Some((address, verdict))
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
                let (due, mut banned) = this.scan(now);

                let refreshed: Vec<_> = stream::iter(due)
                    .map(|address| this.refresh(*address))
                    .buffer_unordered(MAX_CONCURRENT_LOOKUPS)
                    .collect()
                    .await;

                for (address, verdict) in refreshed.into_iter().flatten() {
                    banned += this.store(address, verdict);
                }
                Metrics::currently_banned(banned);
            }
        });
    }
}

/// Logs and swallows backend errors so callers can OR successful results.
async fn fetch_one(backend: &dyn Backend, address: Address) -> Option<bool> {
    let start = Instant::now();
    let result = backend.fetch(address).await;
    Metrics::lookup(
        backend.name(),
        result.as_ref().copied().map_err(|_| ()),
        start.elapsed(),
    );
    match result {
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

    const EXPIRY: CacheExpiry = CacheExpiry {
        not_banned: Duration::from_mins(5),
        banned: Duration::from_hours(24),
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
        let cached = Cached::new(vec![Box::new(backend)], 100, EXPIRY).unwrap();
        (cached, calls, failing)
    }

    #[tokio::test]
    async fn failed_lookup_is_cached_until_retry_succeeds() {
        let (cached, calls, failing) = setup(true);
        let address = Address::repeat_byte(1);
        let addresses = HashSet::from([address]);

        // A failed lookup is cached; a second check is served from cache
        // instead of fetching inline again.
        assert!(cached.check(&addresses).await.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(cached.check(&addresses).await.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // The unknown entry stays due for a background retry until a lookup
        // succeeds.
        assert_eq!(cached.scan(Instant::now()).0.len(), 1);
        failing.store(false, Ordering::SeqCst);
        let (address, verdict) = cached.refresh(address).await.unwrap();
        cached.store(address, verdict);
        assert!(cached.scan(Instant::now()).0.is_empty());
    }

    /// Bans everything in its set.
    struct FixedBackend(HashSet<Address>);

    #[async_trait]
    impl Backend for FixedBackend {
        async fn fetch(&self, address: Address) -> Result<bool, BackendError> {
            Ok(self.0.contains(&address))
        }

        fn name(&self) -> &'static str {
            "fixed"
        }
    }

    #[tokio::test]
    async fn banned_verdicts_are_kept_longer() {
        let banned = Address::repeat_byte(1);
        let clean = Address::repeat_byte(2);
        let backend = FixedBackend(HashSet::from([banned]));
        let cached = Cached::new(vec![Box::new(backend)], 100, EXPIRY).unwrap();
        cached.check(&HashSet::from([banned, clean])).await;
        let start = Instant::now();

        let (due, _) = cached.scan(start + EXPIRY.not_banned);
        assert_eq!(due, vec![Arc::new(clean)]);

        let (due, _) = cached.scan(start + EXPIRY.banned);
        assert_eq!(due.len(), 2);
    }
}
