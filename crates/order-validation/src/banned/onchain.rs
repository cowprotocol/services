//! Chainalysis Oracle-backed sanctioned-address fetcher.
//!
//! Queries the on-chain `isSanctioned` view. Pure fetcher — caching and
//! background refresh are provided by the [`super::cached::Cached`] wrapper.

use {
    super::cached::{Backend, BackendError},
    alloy_primitives::Address,
    async_trait::async_trait,
    contracts::ChainalysisOracle,
};

pub(super) struct Onchain {
    contract: ChainalysisOracle::Instance,
}

impl Onchain {
    pub(super) fn new(contract: ChainalysisOracle::Instance) -> Self {
        Self { contract }
    }
}

#[async_trait]
impl Backend for Onchain {
    async fn fetch(&self, address: Address) -> Result<bool, BackendError> {
        Ok(self.contract.isSanctioned(address).call().await?)
    }

    fn name(&self) -> &'static str {
        "chainalysis"
    }
}
