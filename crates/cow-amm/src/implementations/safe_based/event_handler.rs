use {
    crate::implementations::safe_based::cow_amm::CowAmm,
    contracts::cow_amm_constant_product_factory::Event,
    shared::impl_event_retrieving,
    std::sync::Arc,
};

impl_event_retrieving! {
    pub Contract for contracts::cow_amm_constant_product_factory
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
                let cow_amm = Arc::new(CowAmm::new(
                    deployed.amm,
                    [deployed.token_0, deployed.token_1],
                ));
                Some(cow_amm)
            }
        }
    }
}
