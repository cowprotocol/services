//! Banned user detection: hardcoded list + optional Hermod (zeroShadow).

mod hermod;
mod metrics;

pub use hermod::Config as HermodConfig;
use {self::hermod::Client as Hermod, alloy_primitives::Address, std::collections::HashSet};

/// A list of banned users and optional registries that can be checked.
pub struct Users {
    list: HashSet<Address>,
    remote: Option<Hermod>,
}

impl Users {
    /// Builds the validator from a hardcoded list and an optional Hermod
    /// backend. `cache_max_size` caps the cached Hermod address signatures.
    pub fn new(
        hermod: Option<HermodConfig>,
        banned_users: Vec<Address>,
        cache_max_size: u64,
    ) -> Self {
        Self {
            list: HashSet::from_iter(banned_users),
            remote: hermod.map(|config| Hermod::new(config, cache_max_size)),
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

    /// Returns the subset of `addresses` that are banned. Addresses not in
    /// the hardcoded list are checked against Hermod.
    pub async fn banned(&self, addresses: impl IntoIterator<Item = Address>) -> HashSet<Address> {
        let mut banned = HashSet::new();

        let need_lookup = addresses
            .into_iter()
            .filter(|address| {
                if address.is_zero() {
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
