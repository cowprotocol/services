//! Banned user detection for order validation.
//!
//! Checks if addresses are banned using a hardcoded list and optionally the
//! Chainalysis Oracle on-chain registry and/or the Hermod (zeroShadow) agent.
//! Remote check results are cached (1-hour expiry, LRU eviction) with
//! background refresh every 60 seconds.

mod hermod;

pub use hermod::HermodConfig;
use {
    alloy_primitives::Address,
    contracts::ChainalysisOracle,
    futures::{
        FutureExt,
        StreamExt,
        future::{BoxFuture, join_all},
        stream,
    },
    moka::sync::Cache,
    std::{
        collections::HashSet,
        fmt::Debug,
        future::Future,
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// Caps the number of in-flight per-address fetches so a large batch of cache
/// misses (or a large maintenance refresh) does not burst the backend.
pub(crate) const MAX_CONCURRENT_LOOKUPS: usize = 10;
const CACHE_EXPIRY: Duration = Duration::from_secs(60 * 60);
const MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(60);

/// A list of banned users and optional registries that can be checked.
pub struct Users {
    list: HashSet<Address>,
    onchain: Option<Arc<Onchain>>,
    hermod: Option<Arc<hermod::Hermod>>,
}

#[derive(Clone)]
pub(crate) struct UserMetadata {
    pub(crate) is_banned: bool,
    pub(crate) last_updated: Instant,
}

/// A remote banned-user source backed by a cache. Each implementation only
/// needs to provide the underlying lookup; the shared `check` flow takes
/// care of cache hit/miss handling and writing fresh results back.
pub(crate) trait Backend: Send + Sync + 'static {
    type Error: Debug + Send;

    /// Asks the underlying source whether the given address is banned.
    fn fetch(&self, address: Address) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    fn cache(&self) -> &Cache<Address, UserMetadata>;

    fn name(&self) -> &'static str;

    /// Checks the given addresses against this backend and inserts any
    /// reported hits into `banned`. Addresses already in `banned` are skipped
    /// to avoid an unnecessary lookup.
    async fn check(&self, addresses: &HashSet<Address>, banned: &mut HashSet<Address>) {
        let mut need_lookup = Vec::new();
        for address in addresses {
            if banned.contains(address) {
                continue;
            }
            match self.cache().get(address) {
                Some(metadata) => {
                    metadata.is_banned.then(|| banned.insert(*address));
                }
                None => need_lookup.push(*address),
            }
        }

        let to_cache: Vec<_> = stream::iter(need_lookup)
            .map(|address| async move { (address, self.fetch(address).await) })
            .buffer_unordered(MAX_CONCURRENT_LOOKUPS)
            .collect()
            .await;

        let now = Instant::now();
        for (address, result) in to_cache {
            match result {
                Ok(is_banned) => {
                    self.cache().insert(
                        address,
                        UserMetadata {
                            is_banned,
                            last_updated: now,
                        },
                    );
                    is_banned.then(|| banned.insert(address));
                }
                Err(err) => {
                    tracing::warn!(
                        backend = self.name(),
                        ?err,
                        ?address,
                        "failed to fetch banned status",
                    );
                }
            }
        }
    }
}

/// Onchain banned user checker using Chainalysis Oracle with caching and
/// background refresh. Maintains a size-bounded LRU cache with periodic
/// maintenance to refresh expired entries.
struct Onchain {
    contract: ChainalysisOracle::Instance,
    cache: Cache<Address, UserMetadata>,
}

impl Onchain {
    pub fn new(contract: ChainalysisOracle::Instance, cache_max_size: u64) -> Arc<Self> {
        let onchain = Arc::new(Self {
            contract,
            cache: Cache::builder().max_capacity(cache_max_size).build(),
        });

        onchain.clone().spawn_maintenance_task();

        onchain
    }

    fn expired_data(&self, start: Instant) -> Vec<(Arc<Address>, UserMetadata)> {
        self.cache
            .iter()
            .filter_map(|(address, metadata)| {
                let expired = start
                    .checked_duration_since(metadata.last_updated)
                    .unwrap_or_default()
                    >= CACHE_EXPIRY - MAINTENANCE_TIMEOUT;
                expired.then_some((address, metadata))
            })
            .collect()
    }

    async fn determine_status(
        &self,
        address: Address,
        metadata: UserMetadata,
    ) -> Option<(Address, UserMetadata)> {
        match self.fetch(address).await {
            Ok(is_banned) => Some((
                address,
                UserMetadata {
                    is_banned,
                    ..metadata
                },
            )),
            Err(err) => {
                tracing::warn!(
                    ?address,
                    ?err,
                    "unable to determine banned status in the background task",
                );
                None
            }
        }
    }

    fn insert_many_into_cache(&self, addresses: impl Iterator<Item = (Address, UserMetadata)>) {
        let now = Instant::now();
        for (address, metadata) in addresses {
            self.cache.insert(
                address,
                UserMetadata {
                    last_updated: now,
                    ..metadata
                },
            );
        }
    }

    fn spawn_maintenance_task(self: Arc<Self>) {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(MAINTENANCE_TIMEOUT);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let start = Instant::now();
                let expired_data = self.expired_data(start);

                let results = stream::iter(expired_data)
                    .map(|(address, metadata)| self.determine_status(*address, metadata))
                    .buffer_unordered(MAX_CONCURRENT_LOOKUPS)
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .flatten();

                self.insert_many_into_cache(results);
            }
        });
    }
}

impl Backend for Onchain {
    type Error = alloy_contract::Error;

    async fn fetch(&self, address: Address) -> Result<bool, Self::Error> {
        self.contract.isSanctioned(address).call().await
    }

    fn cache(&self) -> &Cache<Address, UserMetadata> {
        &self.cache
    }

    fn name(&self) -> &'static str {
        "chainalysis"
    }
}

impl Users {
    /// Creates a new `Users` instance that checks the hardcoded list and uses
    /// the given `web3` instance to determine whether an onchain registry of
    /// banned addresses is available.
    pub fn new(
        contract: Option<ChainalysisOracle::Instance>,
        hermod: Option<HermodConfig>,
        banned_users: Vec<Address>,
        cache_max_size: u64,
    ) -> Self {
        Self {
            list: HashSet::from_iter(banned_users),
            onchain: contract.map(|instance| Onchain::new(instance, cache_max_size)),
            hermod: hermod.map(|config| hermod::Hermod::new(config, cache_max_size)),
        }
    }

    /// Creates a new `Users` instance that passes all addresses.
    pub fn none() -> Self {
        Self {
            list: HashSet::new(),
            onchain: None,
            hermod: None,
        }
    }

    /// Creates a new `Users` instance that passes all addresses except for the
    /// ones in `list`.
    pub fn from_set(list: HashSet<Address>) -> Self {
        Self {
            list,
            onchain: None,
            hermod: None,
        }
    }

    /// Returns a subset of addresses from the input iterator which are banned.
    ///
    /// On cache-misses, it will use the Chainalysis oracle and/or the Hermod
    /// agent to determine status.
    pub async fn banned(&self, addresses: impl IntoIterator<Item = Address>) -> HashSet<Address> {
        let mut banned = HashSet::new();

        let need_lookup = addresses
            .into_iter()
            .filter(|address| {
                if self.list.contains(address) {
                    banned.insert(*address);
                    false
                } else {
                    true
                }
            })
            // Need to collect here to make sure filter gets executed and we insert addresses
            .collect::<HashSet<_>>();

        let lookups: Vec<BoxFuture<'_, HashSet<Address>>> = [
            self.onchain
                .as_deref()
                .map(|b| check_into_new(b, &need_lookup).boxed()),
            self.hermod
                .as_deref()
                .map(|b| check_into_new(b, &need_lookup).boxed()),
        ]
        .into_iter()
        .flatten()
        .collect();

        for found in join_all(lookups).await {
            banned.extend(found);
        }

        banned
    }
}

/// Runs `backend.check` against a fresh result set so multiple backends can be
/// driven concurrently without sharing a `&mut HashSet`.
async fn check_into_new<B: Backend>(backend: &B, addresses: &HashSet<Address>) -> HashSet<Address> {
    let mut out = HashSet::new();
    backend.check(addresses, &mut out).await;
    out
}
