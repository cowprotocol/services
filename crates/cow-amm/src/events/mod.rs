pub mod cow_amm;
pub mod cow_amm_constant_product_factory;

use crate::{
    cow_amm::CowAmmHandler,
    cow_amm_constant_product_factory::CowAmmConstantProductFactoryHandler,
    indexer::CowAmmRegistry,
};

#[derive(PartialEq)]
pub enum Event {
    CowAmmConstantProductFactoryEvent(contracts::cow_amm_constant_product_factory::Event),
    CowAmmEvent(contracts::cow_amm::Event),
}

impl From<ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>> for Event {
    fn from(event: ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>) -> Self {
        Self::CowAmmConstantProductFactoryEvent(event.data)
    }
}

impl From<ethcontract::Event<contracts::cow_amm::Event>> for Event {
    fn from(event: ethcontract::Event<contracts::cow_amm::Event>) -> Self {
        Self::CowAmmEvent(event.data)
    }
}

impl Event {
    pub async fn apply_event(&self, cow_amms: &mut CowAmmRegistry) -> anyhow::Result<()> {
        match self {
            Self::CowAmmConstantProductFactoryEvent(event) => {
                CowAmmConstantProductFactoryHandler::apply_event(event.clone(), cow_amms).await
            }
            Self::CowAmmEvent(event) => CowAmmHandler::apply_event(event.clone(), cow_amms).await,
        }
    }
}
