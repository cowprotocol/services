use {
    crate::{cow_amm::CowAmm, indexer::CowAmmRegistry},
    shared::impl_event_retrieving,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_constant_product_factory
}

pub struct CowAmmConstantProductFactoryHandler;

impl CowAmmConstantProductFactoryHandler {
    /// Apply the event to the given CoW AMM registry
    pub async fn apply_event(
        event: &contracts::cow_amm_constant_product_factory::Event,
        cow_amms: &mut CowAmmRegistry,
    ) -> anyhow::Result<()> {
        match &event {
            contracts::cow_amm_constant_product_factory::Event::ConditionalOrderCreated(_)
            | contracts::cow_amm_constant_product_factory::Event::TradingDisabled(_) => {
                // We purposely ignore these events
            }
            contracts::cow_amm_constant_product_factory::Event::Deployed(deployed) => {
                let new_cow_amm = CowAmm::new(deployed.amm, &[deployed.token_0, deployed.token_1]);
                cow_amms.insert(deployed.amm, new_cow_amm);
            }
        }
        Ok(())
    }
}
