use {crate::indexer::CowAmmRegistry, shared::impl_event_retrieving};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm
}

pub struct CowAmmHandler;

impl CowAmmHandler {
    /// Apply the event to the given CoW AMM registry
    pub async fn apply_event(
        event: contracts::cow_amm::Event,
        _cow_amms: &mut CowAmmRegistry,
    ) -> anyhow::Result<()> {
        match &event {
            contracts::cow_amm::Event::TradingDisabled(_) => {
                // How can I get the amm address?
            }
            contracts::cow_amm::Event::TradingEnabled(_) => {
                // How can I get the amm address?
            }
        }
        Ok(())
    }
}
