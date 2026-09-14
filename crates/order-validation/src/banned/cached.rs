//! Shared cache fronting every configured banned-user backend. Stores one
//! entry per address (not per address × backend) since callers only ask
//! "banned by anyone?"; backends stay as pure fetchers.

use {
    super::{Source, metrics::Metrics},
    alloy_primitives::Address,
    async_trait::async_trait,
    enumset::EnumSet,
    futures::{StreamExt, future::join_all, stream},
    moka::sync::Cache,
    std::{
        collections::HashSet,
        sync::{Arc, Weak},
        time::{Duration, Instant},
    },
};

/// Width of the per-source tallies the maintenance task keeps.
const SOURCE_COUNT: usize = EnumSet::<Source>::variant_count() as usize;

/// Caps in-flight fetches so a large miss batch can't burst the backends.
const MAX_CONCURRENT_LOOKUPS: usize = 10;
const CACHE_EXPIRY: Duration = Duration::from_secs(60 * 60);
const MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, PartialEq)]
enum Verdict {
    /// Banned, credited to every backend that reported it.
    Banned(EnumSet<Source>),
    NotBanned,
    /// Every lookup failed. Treated as not banned until a maintenance-task
    /// retry succeeds.
    Unknown,
}

impl Verdict {
    fn sources(self) -> EnumSet<Source> {
        match self {
            Self::Banned(sources) => sources,
            Self::NotBanned | Self::Unknown => EnumSet::empty(),
        }
    }
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

    fn source(&self) -> Source;
}

/// Single cache fronting every configured backend. A miss fans out to every
/// backend in parallel and stores the OR of the results.
pub(super) struct Cached {
    backends: Vec<Box<dyn Backend>>,
    /// Resolved once so the per-entry accounting never dispatches through the
    /// backends.
    sources: EnumSet<Source>,
    cache: Cache<Address, Entry>,
}

impl Cached {
    /// Returns `None` when no backends are configured.
    pub(super) fn new(backends: Vec<Box<dyn Backend>>, max_capacity: u64) -> Option<Arc<Self>> {
        if backends.is_empty() {
            return None;
        }
        let sources = backends.iter().map(|backend| backend.source()).collect();
        let cached = Arc::new(Self {
            backends,
            sources,
            cache: Cache::builder().max_capacity(max_capacity).build(),
        });
        for source in sources {
            Metrics::currently_banned(source, 0);
        }
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
                    (!entry.verdict.sources().is_empty()).then(|| banned.insert(*address));
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

        for (address, verdict) in fetched {
            if !verdict.sources().is_empty() {
                banned.insert(address);
            }
            self.store(address, verdict);
        }

        banned
    }

    /// Caches `verdict`, reporting bans the cache did not already know about.
    /// Returns the sources the cache credited before the write.
    fn store(&self, address: Address, verdict: Verdict) -> EnumSet<Source> {
        let known = self.known_sources(address);
        for source in verdict.sources() - known {
            Metrics::detected(source);
        }
        self.cache.insert(address, Entry::new(verdict));
        known
    }

    fn known_sources(&self, address: Address) -> EnumSet<Source> {
        self.cache
            .get(&address)
            .map(|entry| entry.verdict.sources())
            .unwrap_or_default()
    }

    /// [`Verdict::Banned`] as soon as any backend confirms a ban, since a
    /// failure elsewhere must not mask a positive hit, credited to every
    /// backend that confirmed it. A backend that failed keeps what it last
    /// reported, so an outage does not re-credit an existing ban to the
    /// backends that stayed up for the hour the entry lives.
    /// [`Verdict::Unknown`] means no confirmation and at least one failure.
    async fn fetch_all(&self, address: Address) -> Verdict {
        let results = join_all(self.backends.iter().map(|backend| async move {
            (backend.source(), fetch_one(backend.as_ref(), address).await)
        }))
        .await;

        let (mut confirmed, mut failed) = (EnumSet::empty(), EnumSet::empty());
        for (source, banned) in results {
            match banned {
                Some(true) => confirmed |= source,
                Some(false) => (),
                None => failed |= source,
            }
        }

        if confirmed.is_empty() {
            return if failed.is_empty() {
                Verdict::NotBanned
            } else {
                Verdict::Unknown
            };
        }
        if failed.is_empty() {
            return Verdict::Banned(confirmed);
        }
        Verdict::Banned(confirmed | (failed & self.known_sources(address)))
    }

    /// Walks the cache once, returning the entries due for a refresh and how
    /// many entries are currently banned. Due are the entries close enough to
    /// expiry that the next maintenance tick may miss the window, plus
    /// [`Verdict::Unknown`] entries awaiting a retry.
    fn scan(&self, now: Instant) -> (Vec<Arc<Address>>, [i64; SOURCE_COUNT]) {
        let mut due = Vec::new();
        let mut banned = [0; SOURCE_COUNT];
        for (address, entry) in self.cache.iter() {
            for source in entry.verdict.sources() {
                banned[source as usize] += 1;
            }
            let refresh = entry.verdict == Verdict::Unknown
                || now
                    .checked_duration_since(entry.last_updated)
                    .unwrap_or_default()
                    >= CACHE_EXPIRY - MAINTENANCE_TIMEOUT;
            if refresh {
                due.push(address);
            }
        }
        (due, banned)
    }

    /// `None` (existing entry preserved) when `fetch_all` is uncertain — no
    /// positive confirmation and at least one backend failed.
    async fn refresh(&self, address: Address) -> Option<(Address, Verdict)> {
        match self.fetch_all(address).await {
            Verdict::Unknown => None,
            verdict => Some((address, verdict)),
        }
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
                    let known = this.store(address, verdict);
                    for source in verdict.sources() - known {
                        banned[source as usize] += 1;
                    }
                    for source in known - verdict.sources() {
                        banned[source as usize] -= 1;
                    }
                }
                for source in this.sources {
                    Metrics::currently_banned(source, banned[source as usize]);
                }
            }
        });
    }
}

/// Logs and swallows backend errors so callers can OR successful results.
async fn fetch_one(backend: &dyn Backend, address: Address) -> Option<bool> {
    let start = Instant::now();
    let result = backend.fetch(address).await;
    Metrics::lookup(
        backend.source(),
        result.as_ref().copied().map_err(|_| ()),
        start.elapsed(),
    );
    match result {
        Ok(banned) => Some(banned),
        Err(err) => {
            tracing::warn!(
                backend = backend.source().as_str(),
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
        source: Source,
        banned: bool,
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
                Ok(self.banned)
            }
        }

        fn source(&self) -> Source {
            self.source
        }
    }

    fn backend(source: Source, banned: bool, fail: &Arc<AtomicBool>) -> Box<dyn Backend> {
        Box::new(FlakyBackend {
            source,
            banned,
            calls: Arc::new(AtomicUsize::new(0)),
            fail: fail.clone(),
        })
    }

    fn setup(fail: bool) -> (Arc<Cached>, Arc<AtomicUsize>, Arc<AtomicBool>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let failing = Arc::new(AtomicBool::new(fail));
        let backend = FlakyBackend {
            source: Source::Hermod,
            banned: false,
            calls: calls.clone(),
            fail: failing.clone(),
        };
        let cached = Cached::new(vec![Box::new(backend)], 100).unwrap();
        (cached, calls, failing)
    }

    #[tokio::test]
    async fn every_reporting_backend_is_credited() {
        let up = Arc::new(AtomicBool::new(false));
        let cached = Cached::new(
            vec![
                backend(Source::Chainalysis, true, &up),
                backend(Source::Hermod, true, &up),
            ],
            100,
        )
        .unwrap();

        assert_eq!(
            cached.fetch_all(Address::repeat_byte(1)).await.sources(),
            Source::Chainalysis | Source::Hermod
        );
    }

    #[tokio::test]
    async fn a_failing_backend_keeps_the_ban_it_last_reported() {
        let up = Arc::new(AtomicBool::new(false));
        let chainalysis_down = Arc::new(AtomicBool::new(false));
        let cached = Cached::new(
            vec![
                backend(Source::Chainalysis, true, &chainalysis_down),
                backend(Source::Hermod, true, &up),
            ],
            100,
        )
        .unwrap();
        let address = Address::repeat_byte(1);

        let verdict = cached.fetch_all(address).await;
        cached.store(address, verdict);

        chainalysis_down.store(true, Ordering::SeqCst);
        assert_eq!(
            cached.fetch_all(address).await.sources(),
            Source::Chainalysis | Source::Hermod,
            "an outage must not re-credit the ban to the backends that stayed up"
        );
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
}
