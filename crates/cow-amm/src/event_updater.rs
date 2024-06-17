use {
    crate::Indexer,
    ethrpc::current_block::BlockRetrieving,
    shared::{event_handling::EventHandler, maintenance::Maintaining},
    std::sync::Arc,
    tokio::sync::Mutex,
};

pub struct EventUpdater(
    Mutex<EventHandler<crate::cow_amm_constant_product_factory::Contract, crate::Indexer>>,
);

impl EventUpdater {
    pub async fn build(
        block_retriever: Arc<dyn BlockRetrieving>,
        indexer: &Indexer,
        cow_amm_factory: &contracts::CowAmmConstantProductFactory,
    ) -> Arc<dyn Maintaining> {
        let contract =
            crate::cow_amm_constant_product_factory::Contract::new(cow_amm_factory.clone());
        let event_handler = EventHandler::new(block_retriever, contract, indexer.clone(), None);
        Arc::new(Self(Mutex::new(event_handler)))
    }
}

#[async_trait::async_trait]
impl Maintaining for EventUpdater {
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        self.0.run_maintenance().await
    }

    fn name(&self) -> &str {
        "CowAmmEventUpdater"
    }
}
