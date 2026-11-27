//! Banned user detection: hardcoded list + optional Hermod (zeroShadow)
//! behind a cache layer. An address is any type that works as bytes.

mod cached;
mod config;
mod hermod;
mod metrics;

pub use config::{Config, HermodConfig};
use {
    self::{cached::Cached, hermod::Client as Hermod},
    std::{collections::HashSet, hash::Hash, sync::Arc},
};

/// An account address: a value type that gives its bytes.
pub trait Address: AsRef<[u8]> + Copy + Eq + Hash + Send + Sync + 'static {}

impl<T: AsRef<[u8]> + Copy + Eq + Hash + Send + Sync + 'static> Address for T {}

/// A list of banned users and optional registries that can be checked.
pub struct Users<A: Address> {
    list: HashSet<A>,
    remote: Option<Arc<Cached<A>>>,
}

impl<A: Address> Users<A> {
    /// Builds the validator from the hardcoded list and the optional Hermod
    /// backend of `config`. `max_cache_size` caps the cached Hermod address
    /// signatures and the cached verdicts.
    pub fn new(config: Config<A>) -> Self {
        let max_cache_size = config.max_cache_size.get() as u64;
        Self {
            list: HashSet::from_iter(config.addresses),
            remote: config.hermod.map(|hermod| {
                let (refresh_interval, max_idle) = (hermod.refresh_interval, hermod.max_idle);
                Cached::new(
                    Hermod::new(hermod, max_cache_size),
                    max_cache_size,
                    refresh_interval,
                    max_idle,
                )
            }),
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
    pub fn from_set(list: HashSet<A>) -> Self {
        Self { list, remote: None }
    }

    /// Returns the subset of `addresses` that are banned. Addresses not in
    /// the hardcoded list are checked against the cached Hermod verdicts.
    pub async fn banned(&self, addresses: impl IntoIterator<Item = A>) -> HashSet<A> {
        let mut banned = HashSet::new();

        let need_lookup = addresses
            .into_iter()
            .filter(|address| {
                if address.as_ref().iter().all(|byte| *byte == 0) {
                    // We use the zero/burn address for some quotes, there's no
                    // point in checking if its banned
                    return false;
                }
                if self.list.contains(address) {
                    banned.insert(*address);
                    false
                } else {
                    true
                }
            })
            // Need to collect here to make sure filter gets executed and we insert addresses
            .collect::<HashSet<_>>();

        if let Some(remote) = &self.remote
            && !need_lookup.is_empty()
        {
            banned.extend(remote.banned(&need_lookup).await);
        }

        banned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bans_listed_addresses() {
        let (listed, other) = ([1u8; 20], [2u8; 20]);
        let users = Users::from_set(HashSet::from([listed]));
        assert_eq!(users.banned([listed, other]).await, HashSet::from([listed]));
    }

    #[tokio::test]
    async fn never_bans_the_zero_address() {
        let zero = [0u8; 20];
        let users = Users::from_set(HashSet::from([zero]));
        assert!(users.banned([zero]).await.is_empty());
    }
}
