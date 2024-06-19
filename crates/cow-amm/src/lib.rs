mod event_updater;
mod implementations;
mod indexer;

use {
    crate::indexer::CowAmmRegistry,
    ethcontract::{common::DeploymentInformation, Address},
};
pub use {event_updater::EventUpdater, implementations::safe_based::*, indexer::Indexer};

pub trait CowAmm: Send + Sync {
    /// Address of the CoW AMM.
    /// Can be used by the autopilot to build the list of accepted cow amms.
    fn address(&self) -> &Address;

    /// Returns the list of tokens traded by this pool.
    /// Can be used by the autopilot to build the list of native token prices to
    /// query.
    fn traded_tokens(&self) -> &[Address];
}

#[async_trait::async_trait]
pub trait ContractHandler<E>: Send + Sync {
    /// Information about when a contract instance was deployed
    fn deployment_information(&self) -> Option<DeploymentInformation>;

    /// Apply the event to the given CoW AMM registry
    async fn apply_event(
        &self,
        block_number: u64,
        event: &E,
        cow_amms: &mut CowAmmRegistry,
    ) -> anyhow::Result<()>;
}
