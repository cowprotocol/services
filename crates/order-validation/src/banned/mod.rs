//! Banned user detection: hardcoded list + optional Chainalysis Oracle
//! and/or Hermod (zeroShadow). Remote sources share one cache layer.

mod cached;
mod hermod;
mod onchain;

pub use hermod::Config as HermodConfig;
use {
    self::{
        cached::{Backend, Cached},
        hermod::Client as Hermod,
        onchain::Onchain,
    },
    alloy_primitives::{Address, map::AddressHashSet},
    contracts::ChainalysisOracle,
    std::sync::Arc,
};

/// A list of banned users and optional registries that can be checked.
pub struct Users {
    list: AddressHashSet,
    remote: Option<Arc<Cached>>,
}

impl Users {
    /// Builds the validator from a hardcoded list and optional remote backends.
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
            list: AddressHashSet::from_iter(banned_users),
            remote: Cached::new(backends, cache_max_size),
        }
    }

    /// Creates a new `Users` instance that passes all addresses.
    pub fn none() -> Self {
        Self {
            list: AddressHashSet::default(),
            remote: None,
        }
    }

    /// Creates a new `Users` instance that passes all addresses except for the
    /// ones in `list`.
    pub fn from_set(list: AddressHashSet) -> Self {
        Self { list, remote: None }
    }

    /// Returns the subset of `addresses` that are banned. Cache misses hit
    /// the configured remote sources.
    pub async fn banned(&self, addresses: impl IntoIterator<Item = Address>) -> AddressHashSet {
        let mut banned = AddressHashSet::default();

        let need_lookup = addresses
            .into_iter()
            .filter(|address| {
                if address.is_zero() {
                    // We use the zero/burn address for some quotes, there's no point in checking if its banned
                    return false
                }
                if self.list.contains(address) {
                    banned.insert(*address);
                    false
                } else {
                    true
                }
            })
            // Need to collect here to make sure filter gets executed and we insert addresses
            .collect::<AddressHashSet>();

        if let Some(remote) = &self.remote {
            banned.extend(remote.check(&need_lookup).await);
        }

        banned
    }
}
