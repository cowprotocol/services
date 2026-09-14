//! Banned user detection: hardcoded list + optional Chainalysis Oracle
//! and/or Hermod (zeroShadow). Remote sources share one cache layer.

mod cached;
mod hermod;
mod metrics;
mod onchain;

pub use hermod::Config as HermodConfig;
use {
    self::{
        cached::{Backend, Cached},
        hermod::Client as Hermod,
        metrics::Metrics,
        onchain::Onchain,
    },
    alloy_primitives::Address,
    contracts::ChainalysisOracle,
    std::{collections::HashSet, sync::Arc},
};

/// Where a ban came from. Doubles as the label of the per-backend metrics, so
/// a backend cannot report a source the gauges never publish.
#[derive(Debug, enumset::EnumSetType)]
enum Source {
    /// The hardcoded deny-list, which is not backed by a registry.
    List,
    Chainalysis,
    Hermod,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Chainalysis => "chainalysis",
            Self::Hermod => "hermod",
        }
    }
}

/// A list of banned users and optional registries that can be checked.
pub struct Users {
    list: HashSet<Address>,
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
        Self::with(
            HashSet::from_iter(banned_users),
            Cached::new(backends, cache_max_size),
        )
    }

    /// Creates a new `Users` instance that passes all addresses.
    pub fn none() -> Self {
        Self::with(HashSet::new(), None)
    }

    /// Creates a new `Users` instance that passes all addresses except for the
    /// ones in `list`.
    pub fn from_set(list: HashSet<Address>) -> Self {
        Self::with(list, None)
    }

    /// The deny-list never changes after construction, so its gauge is
    /// published once instead of by the cache maintenance task.
    fn with(list: HashSet<Address>, remote: Option<Arc<Cached>>) -> Self {
        Metrics::currently_banned(Source::List, i64::try_from(list.len()).unwrap_or(i64::MAX));
        Self { list, remote }
    }

    /// Returns the subset of `addresses` that are banned. Cache misses hit
    /// the configured remote sources.
    pub async fn banned(&self, addresses: impl IntoIterator<Item = Address>) -> HashSet<Address> {
        let mut banned = HashSet::new();

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
            .collect::<HashSet<_>>();

        if let Some(remote) = &self.remote {
            banned.extend(remote.check(&need_lookup).await);
        }

        banned
    }
}
