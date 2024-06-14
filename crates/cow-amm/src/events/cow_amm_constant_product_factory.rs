use {
    crate::indexer::{CowAmm, CowAmmRegistry},
    shared::impl_event_retrieving,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_constant_product_factory
}

pub struct CowAmmConstantProductFactoryHandler;

impl CowAmmConstantProductFactoryHandler {
    /// Apply the event to the given CoW AMM registry
    pub async fn apply_event(
        event: contracts::cow_amm_constant_product_factory::Event,
        cow_amms: &mut CowAmmRegistry,
    ) -> anyhow::Result<()> {
        match &event {
            contracts::cow_amm_constant_product_factory::Event::ConditionalOrderCreated(_) => {
                // Do nothing?
            }
            contracts::cow_amm_constant_product_factory::Event::Deployed(deployed) => {
                let new_cow_amm = CowAmm::new(deployed.amm, &[deployed.token_0, deployed.token_1]);
                cow_amms.insert(deployed.amm, new_cow_amm);
            }
            contracts::cow_amm_constant_product_factory::Event::TradingDisabled(
                trading_disabled,
            ) => {
                cow_amms
                    .get_mut(&trading_disabled.amm)
                    .map(|cow_amm| cow_amm.enable());
            }
        }
        Ok(())
    }
}
