use {
    crate::implementations::standalone::amm::Amm,
    contracts::cow_amm_legacy_helper::Event,
    ethcontract::common::DeploymentInformation,
    ethrpc::Web3,
    shared::impl_event_retrieving,
    std::sync::Arc,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_legacy_helper
}

impl Contract {
    pub fn deployment_block(&self) -> u64 {
        let Some(info) = self.0.deployment_information() else {
            // No deployment info should indicate a test environment => start from genesis.
            return 0;
        };
        match info {
            DeploymentInformation::BlockNumber(block) => block,
            DeploymentInformation::TransactionHash(_) => {
                panic!("no block number in deployment info")
            }
        }
    }
}

#[async_trait::async_trait]
impl crate::Deployment for Event {
    /// Returns the AMM deployed in the given Event.
    async fn deployed_amm(&self, web3: &Web3) -> Option<Arc<dyn crate::CowAmm>> {
        match &self {
            Event::CowammpoolCreated(data) => {
                let address = data.amm;
                let helper = contracts::CowAmmLegacyHelper::deployed(web3).await.unwrap();
                let tokens = helper.tokens(address).call().await.unwrap();
                let cow_amm = Arc::new(Amm::new(address, tokens));
                Some(cow_amm)
            }
        }
    }
}
