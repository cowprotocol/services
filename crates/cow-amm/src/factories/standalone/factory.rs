use {
    crate::implementations::standalone::amm::Amm,
    anyhow::Result,
    contracts::cow_amm_legacy_helper::Event,
    ethcontract::common::DeploymentInformation,
    ethrpc::Web3,
    shared::impl_event_retrieving,
    std::sync::Arc,
    contracts::CowAmmLegacyHelper,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_legacy_helper
}

impl Contract {
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
impl crate::Deployment for Event {
    /// Returns the AMM deployed in the given Event.
    async fn deployed_amm(&self, helper: &CowAmmLegacyHelper) -> Result<Option<Arc<dyn crate::CowAmm>>> {
        match &self {
            Event::CowammpoolCreated(data) => Ok(Some(Arc::new(Amm::new(data.amm, helper).await?))),
        }
    }
}
