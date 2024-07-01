use {
    crate::{factories::Deployment, Amm},
    anyhow::Result,
    contracts::{cow_amm_legacy_helper::Event, CowAmmLegacyHelper},
    ethcontract::common::DeploymentInformation,
    shared::impl_event_retrieving,
};

impl_event_retrieving! {
    pub Factory for contracts::cow_amm_legacy_helper
}

impl Factory {
    pub fn start_indexing_at(&self) -> u64 {
        match self.0.deployment_information().unwrap() {
            DeploymentInformation::TransactionHash(_) => {
                panic!("no block number in deployment info")
            }
            // The helper contract emitted `Deployment` events for all
            // pools known at the time in its constructor.
            // To actually index all these events correctly we need to
            // start indexing 1 block before.
            DeploymentInformation::BlockNumber(block) => block - 1,
        }
    }
}

#[async_trait::async_trait]
impl Deployment for Event {
    /// Returns the AMM deployed in the given Event.
    async fn deployed_amm(&self, helper: &CowAmmLegacyHelper) -> Result<Option<Amm>> {
        match &self {
            Event::CowammpoolCreated(data) => Ok(Some(Amm::new(data.amm, helper).await?)),
        }
    }
}
