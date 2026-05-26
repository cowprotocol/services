//! Chainalysis Oracle-backed sanctioned-address checker.
//!
//! Queries the on-chain `isSanctioned` view. Mirrors the structure of the
//! Hermod checker: same cache, same background refresh task.

use {
    super::{Backend, UserMetadata},
    alloy_primitives::Address,
    contracts::ChainalysisOracle,
    moka::sync::Cache,
    std::sync::Arc,
};

/// Onchain banned user checker using Chainalysis Oracle with caching and
/// background refresh. Maintains a size-bounded LRU cache with periodic
/// maintenance to refresh expired entries.
pub(super) struct Onchain {
    contract: ChainalysisOracle::Instance,
    cache: Cache<Address, UserMetadata>,
}

impl Onchain {
    pub(super) fn new(contract: ChainalysisOracle::Instance, cache_max_size: u64) -> Arc<Self> {
        let onchain = Arc::new(Self {
            contract,
            cache: Cache::builder().max_capacity(cache_max_size).build(),
        });

        onchain.clone().spawn_maintenance_task();

        onchain
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
