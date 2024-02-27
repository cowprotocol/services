use {
    contracts::ChainalysisOracle,
    ethcontract::{errors::MethodError, H160},
    ethrpc::Web3,
    std::{collections::HashSet, sync::RwLock, time::Duration},
    ttl_cache::TtlCache,
};

/// A list of banned users and an optional registry that can be checked onchain.
pub struct Users {
    list: HashSet<H160>,
    onchain: Option<Onchain>,
}

struct Onchain {
    contract: ChainalysisOracle,
    cache: RwLock<TtlCache<H160, bool>>,
}

const TTL: Duration = Duration::from_secs(60 * 60); // 1 hour

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
                    cache: RwLock::new(TtlCache::new(usize::MAX)),
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

    /// Returns `true` if the given address is banned or an error if the result
    /// cannot be determined.
    pub async fn is_banned(&self, address: H160) -> Result<bool, Error> {
        if self.list.contains(&address) {
            return Ok(true);
        }

        if let Some(onchain) = &self.onchain {
            if let Some(result) = onchain.cache.read().expect("unpoisoned").get(&address) {
                return Ok(*result);
            }

            let result = onchain.fetch(address).await?;
            onchain
                .cache
                .write()
                .expect("unpoisoned")
                .insert(address, result, TTL);
            Ok(result)
        } else {
            Ok(false)
        }
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
