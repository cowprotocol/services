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

#[derive(Clone)]
struct UserMetadata {
    is_banned: bool,
    last_updated: Instant,
    #[allow(dead_code)]
    limit_order_participant: bool,
}

impl UserMetadata {
    pub fn with_banned(mut self, banned: bool) -> Self {
        self.is_banned = banned;
        self
    }

    pub fn with_last_updated(mut self, last_updated: Instant) -> Self {
        self.last_updated = last_updated;
        self
    }

    // pub fn with_limit_order_participation(mut self, limit_order_participation:
    // bool) -> Self {     self.limit_order_participation =
    // limit_order_participation;     self
    // }
}

struct Onchain {
    contract: ChainalysisOracle,
    cache: Mutex<HashMap<H160, UserMetadata>>,
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

                let expired_data: Vec<(H160, UserMetadata)> = {
                    let cache = detector.cache.lock().unwrap();
                    let now = Instant::now();
                    cache
                        .iter()
                        .filter_map(|(address, metadata)| {
                            (now.checked_duration_since(metadata.last_updated)
                                .unwrap_or_default()
                                >= maintenance_timeout)
                                .then_some((*address, metadata.clone()))
                        })
                        .collect()
                };

                let results = join_all(expired_data.into_iter().map(|(address, metadata)| {
                    let detector = detector.clone();
                    async move {
                        match detector.fetch(address).await {
                            Ok(result) => Some((address, metadata.with_banned(result))),
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

    fn insert_many_into_cache(&self, addresses: impl Iterator<Item = (H160, UserMetadata)>) {
        let mut cache = self.cache.lock().unwrap();
        let now = Instant::now();
        for (address, metadata) in addresses {
            cache.insert(address, metadata.with_last_updated(now));
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
    pub async fn banned(&self, addresses: impl IntoIterator<Item = (H160, bool)>) -> HashSet<H160> {
        let mut banned = HashSet::new();

        let need_lookup = addresses
            .into_iter()
            .filter(|(address, _)| {
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
                .filter(|(address, _)| {
                    if let Some(metadata) = cache.cache_get(address) {
                        metadata.is_banned.then(|| banned.insert(*address));
                        false
                    } else {
                        true
                    }
                })
                .collect()
        };

        let to_cache = join_all(need_lookup.into_iter().map(
            |(address, limit_order_participant)| async move {
                (
                    address,
                    onchain.fetch(address).await,
                    limit_order_participant,
                )
            },
        ))
        .await;

        let mut cache = onchain.cache.lock().expect("unpoisoned");
        let now = Instant::now();
        for (address, result, limit_order_participant) in to_cache {
            match result {
                Ok(is_banned) => {
                    cache.cache_set(
                        address,
                        UserMetadata {
                            is_banned,
                            last_updated: now,
                            limit_order_participant,
                        },
                    );
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
