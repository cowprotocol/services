use {
    crate::implementations::safe_based::cow_amm::CowAmm,
    contracts::cow_amm_constant_product_factory::Event,
    shared::impl_event_retrieving,
    std::sync::Arc,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_constant_product_factory
}

#[async_trait::async_trait]
impl crate::ContractHandler for Event {
    /// Apply the event to the given CoW AMM registry
    async fn apply_event(&self) -> anyhow::Result<Option<Arc<dyn crate::CowAmm>>> {
        match &self {
            Event::ConditionalOrderCreated(_) | Event::TradingDisabled(_) => {
                // We purposely ignore these events
                return Ok(None);
            }
            Event::Deployed(deployed) => {
                let cow_amm = CowAmm::build(deployed.amm, &[deployed.token_0, deployed.token_1]);
                return Ok(Some(cow_amm));
            }
        }
    }
}
