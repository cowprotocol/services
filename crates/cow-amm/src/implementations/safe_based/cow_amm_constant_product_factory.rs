use {
    crate::{implementations::safe_based::cow_amm::CowAmm, indexer::CowAmmRegistry},
    ethcontract::{common::DeploymentInformation, dyns::DynWeb3},
    shared::impl_event_retrieving,
    std::sync::Arc,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_constant_product_factory
}

pub struct CowAmmConstantProductFactoryHandler {
    contract: contracts::CowAmmConstantProductFactory,
}

impl CowAmmConstantProductFactoryHandler {
    pub fn from_contract(contract: contracts::CowAmmConstantProductFactory) -> Arc<Self> {
        Arc::new(Self { contract })
    }

    pub async fn deployed(web3: &DynWeb3) -> Arc<Self> {
        Arc::new(Self {
            contract: contracts::CowAmmConstantProductFactory::deployed(web3)
                .await
                .expect("Failed to find deployed CowAmmConstantProductFactory"),
        })
    }
}

#[async_trait::async_trait]
impl crate::ContractHandler<contracts::cow_amm_constant_product_factory::Event>
    for CowAmmConstantProductFactoryHandler
{
    fn deployment_information(&self) -> Option<DeploymentInformation> {
        self.contract.deployment_information()
    }

    /// Apply the event to the given CoW AMM registry
    async fn apply_event(
        &self,
        block_number: u64,
        event: &contracts::cow_amm_constant_product_factory::Event,
        cow_amms: &mut CowAmmRegistry,
    ) -> anyhow::Result<()> {
        match &event {
            contracts::cow_amm_constant_product_factory::Event::ConditionalOrderCreated(_)
            | contracts::cow_amm_constant_product_factory::Event::TradingDisabled(_) => {
                // We purposely ignore these events
            }
            contracts::cow_amm_constant_product_factory::Event::Deployed(deployed) => {
                let new_cow_amm =
                    CowAmm::build(deployed.amm, &[deployed.token_0, deployed.token_1]);
                cow_amms.insert(block_number, new_cow_amm);
            }
        }
        Ok(())
    }
}
