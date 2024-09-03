use {
    cached::Cached,
    contracts::ChainalysisOracle,
    ethcontract::{errors::MethodError, futures::future::join_all, H160},
    std::{
        collections::{HashMap, HashSet},
        ops::Div,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
};

/// A list of banned users and an optional registry that can be checked onchain.
pub struct Users {
    list: HashSet<H160>,
    onchain: Option<Arc<Onchain>>,
}

struct Onchain {
    contract: ChainalysisOracle,
    cache: Mutex<HashMap<H160, (Instant, bool)>>,
}

impl Onchain {
    pub fn new(contract: ChainalysisOracle) -> Arc<Self> {
        let onchain = Arc::new(Self {
            contract,
            cache: Default::default(),
        });

        onchain.clone().spawn_maintenance_task();

        onchain
    }

    fn spawn_maintenance_task(self: Arc<Self>) {
        let cache_expiry = Duration::from_secs(60 * 60);
        let maintenance_timeout = cache_expiry.div(10).max(Duration::from_secs(60));
        let detector = Arc::clone(&self);

        tokio::task::spawn(async move {
            loop {
                let start = Instant::now();

                let expired_addresses: Vec<H160> = {
                    let cache = detector.cache.lock().unwrap();
                    let now = Instant::now();
                    cache
                        .iter()
                        .filter_map(|(address, (instant, _))| {
                            (now.checked_duration_since(*instant).unwrap_or_default()
                                >= maintenance_timeout)
                                .then_some(*address)
                        })
                        .collect()
                };

                let results = join_all(expired_addresses.into_iter().map(|address| {
                    let detector = detector.clone();
                    async move {
                        match detector.fetch(address).await {
                            Ok(result) => Some((address, result)),
                            Err(err) => {
                                tracing::warn!(
                                    ?address,
                                    ?err,
                                    "unable to determine banned status in the background task"
                                );
                                None
                            }
                        }
                    }
                }))
                .await
                .into_iter()
                .flatten();

                detector.insert_many_into_cache(results);

                let remaining_sleep = maintenance_timeout
                    .checked_sub(start.elapsed())
                    .unwrap_or_default();
                tokio::time::sleep(remaining_sleep).await;
            }
        });
    }

    fn insert_many_into_cache(&self, addresses: impl Iterator<Item = (H160, bool)>) {
        let mut cache = self.cache.lock().unwrap();
        let now = Instant::now();
        for (address, is_banned) in addresses {
            cache.insert(address, (now, is_banned));
        }
    }
}

impl Users {
    /// Creates a new `Users` instance that checks the hardcoded list and uses
    /// the given `web3` instance to determine whether an onchain registry of
    /// banned addresses is available.
    pub fn new(contract: Option<ChainalysisOracle>, banned_users: Vec<H160>) -> Self {
        Self {
            list: HashSet::from_iter(banned_users),
            onchain: contract.map(Onchain::new),
        }
    }

    /// Creates a new `Users` instance that passes all addresses.
    pub fn none() -> Self {
        Self {
            list: HashSet::new(),
            onchain: None,
        }
    }

    /// Creates a new `Users` instance that passes all addresses except for the
    /// ones in `list`.
    pub fn from_set(list: HashSet<H160>) -> Self {
        Self {
            list,
            onchain: None,
        }
    }

    /// Returns a subset of addresses from the input iterator which are banned.
    pub async fn banned(&self, addresses: impl IntoIterator<Item = H160>) -> HashSet<H160> {
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

        let Some(onchain) = &self.onchain else {
            return banned;
        };
        let need_lookup: Vec<_> = {
            // Scope here to release the lock before the async lookups
            let mut cache = onchain.cache.lock().expect("unpoisoned");
            need_lookup
                .into_iter()
                .filter(|address| {
                    if let Some((_, is_banned)) = cache.cache_get(address) {
                        is_banned.then(|| banned.insert(*address));
                        false
                    } else {
                        true
                    }
                })
                .collect()
        };

        let to_cache = join_all(
            need_lookup
                .into_iter()
                .map(|address| async move { (address, onchain.fetch(address).await) }),
        )
        .await;

        let mut cache = onchain.cache.lock().expect("unpoisoned");
        let now = Instant::now();
        for (address, result) in to_cache {
            match result {
                Ok(is_banned) => {
                    cache.cache_set(address, (now, is_banned));
                    is_banned.then(|| banned.insert(address));
                }
                Err(err) => {
                    tracing::warn!(?err, ?address, "failed to fetch banned status",);
                }
            }
        }
        banned
    }
}

impl Onchain {
    async fn fetch(&self, address: H160) -> Result<bool, Error> {
        Ok(self.contract.is_sanctioned(address).call().await?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to fetch banned users from onchain")]
    Onchain(#[from] MethodError),
}
