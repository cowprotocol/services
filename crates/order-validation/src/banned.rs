use {
    cached::{Cached, TimedCache},
    contracts::ChainalysisOracle,
    ethcontract::{errors::MethodError, futures::future::join_all, H160},
    ethrpc::Web3,
    std::{collections::HashSet, sync::RwLock},
};

/// A list of banned users and an optional registry that can be checked onchain.
pub struct Users {
    list: HashSet<H160>,
    onchain: Option<Onchain>,
}

struct Onchain {
    contract: ChainalysisOracle,
    cache: RwLock<TimedCache<H160, bool>>,
}

const TTL: u64 = 60 * 60; // 1 hour

impl Users {
    /// Creates a new `Users` instance that checks the hardcoded list and uses
    /// the given `web3` instance to determine whether an onchain registry of
    /// banned addresses is available.
    pub async fn new(web3: &Web3, banned_users: Vec<H160>) -> Self {
        Self {
            list: HashSet::from_iter(banned_users),
            onchain: ChainalysisOracle::deployed(web3)
                .await
                .map(|contract| Onchain {
                    contract,
                    cache: RwLock::new(TimedCache::with_lifespan(TTL)),
                })
                .ok(),
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
    pub async fn banned(&self, addresses: impl Iterator<Item = H160>) -> HashSet<H160> {
        let mut banned = HashSet::new();

        let need_lookup = addresses
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

        if let Some(onchain) = &self.onchain {
            let need_lookup: Vec<_> = {
                // Scope here to release the lock before the async lookups
                let mut cache = onchain.cache.write().expect("unpoisoned");
                need_lookup
                    .into_iter()
                    .filter(|address| {
                        if let Some(is_banned) = cache.cache_get(&address) {
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

            let mut cache = onchain.cache.write().expect("unpoisoned");
            for (address, result) in to_cache {
                match result {
                    Ok(is_banned) => {
                        cache.cache_set(address, is_banned);
                        is_banned.then(|| banned.insert(address));
                    }
                    Err(err) => {
                        tracing::warn!("failed to fetch banned status for {}: {}", address, err);
                    }
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
