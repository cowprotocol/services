//! Banned user detection for order validation.
//!
//! Checks if addresses are banned using a hardcoded list and optionally the
//! Chainalysis Oracle on-chain registry and/or the Hermod (zeroShadow) agent.
//! Remote sources sit behind a single shared cache layer ([`cached::Cached`])
//! which provides LRU caching with 1-hour expiry and a background refresh
//! task.

mod cached;
mod hermod;
mod onchain;

pub use hermod::HermodConfig;
use {
    self::{
        cached::{Backend, Cached},
        hermod::Hermod,
        onchain::Onchain,
    },
    alloy_primitives::Address,
    contracts::ChainalysisOracle,
    std::{collections::HashSet, sync::Arc},
};

/// A list of banned users and optional registries that can be checked.
pub struct Users {
    list: HashSet<Address>,
    remote: Option<Arc<Cached>>,
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
        let mut backends: Vec<Box<dyn Backend>> = Vec::new();
        if let Some(contract) = contract {
            backends.push(Box::new(Onchain::new(contract)));
        }
        if let Some(config) = hermod {
            backends.push(Box::new(Hermod::new(config)));
        }
        Self {
            list: HashSet::from_iter(banned_users),
            remote: Cached::new(backends, cache_max_size),
        }
    }

    /// Creates a new `Users` instance that passes all addresses.
    pub fn none() -> Self {
        Self {
            list: HashSet::new(),
            remote: None,
        }
    }

    /// Creates a new `Users` instance that passes all addresses except for the
    /// ones in `list`.
    pub fn from_set(list: HashSet<Address>) -> Self {
        Self { list, remote: None }
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

        if let Some(remote) = &self.remote {
            banned.extend(remote.check(&need_lookup).await);
        }

        banned
    }
}
