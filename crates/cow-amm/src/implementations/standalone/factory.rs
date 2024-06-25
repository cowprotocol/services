use {
    crate::implementations::standalone::amm::Amm,
    contracts::cow_amm_constant_product_factory::Event,
    ethcontract::common::DeploymentInformation,
    shared::impl_event_retrieving,
    std::sync::Arc,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_constant_product_factory
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

impl crate::Deployment for Event {
    /// Returns the AMM deployed in the given Event.
    fn deployed_amm(&self) -> Option<Arc<dyn crate::CowAmm>> {
        match &self {
            Event::ConditionalOrderCreated(_) | Event::TradingDisabled(_) => {
                // We purposely ignore these events
                None
            }
            Event::Deployed(deployed) => {
                let cow_amm =
                    Arc::new(Amm::new(deployed.amm, [deployed.token_0, deployed.token_1]));
                Some(cow_amm)
            }
        }
    }
}
