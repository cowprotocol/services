use {
    contracts::CowAmmConstantProductFactory,
    ethcontract::{common::DeploymentInformation, dyns::DynWeb3},
    ethrpc::current_block::RangeInclusive,
    primitive_types::H160,
    shared::{event_handling::EventStoring, impl_event_retrieving},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
    tokio::sync::RwLock,
};

impl_event_retrieving! {
    pub CowAmmConstantProductFactoryContract for contracts::cow_amm_constant_product_factory
}

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone)]
pub struct Indexer {
    /// Registry with the format: (block number, event)
    registry: Arc<
        RwLock<BTreeMap<u64, contracts::cow_amm_constant_product_factory::event_data::Deployed>>,
    >,
    first_block: u64,
}

impl Indexer {
    pub async fn new(web3: &DynWeb3) -> Self {
        let cow_amm_constant_product_factory = CowAmmConstantProductFactory::deployed(web3)
            .await
            .expect("Failed to find deployed CowAmmConstantProductFactory");
        let first_block = match cow_amm_constant_product_factory
            .deployment_information()
            .expect("Failed to get deployment information")
        {
            DeploymentInformation::BlockNumber(block) => block,
            _ => panic!("Expected block number"),
        };
        Self {
            registry: Arc::new(RwLock::new(BTreeMap::new())),
            first_block,
        }
    }

    pub async fn get_cow_amm_addresses(&self) -> HashSet<H160> {
        self.registry
            .read()
            .await
            .values()
            .map(|event| event.amm)
            .collect()
    }
}

#[async_trait::async_trait]
impl EventStoring<contracts::cow_amm_constant_product_factory::Event> for Indexer {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        // Remove events in the specified range
        let range = *range.start()..=*range.end();
        // Create a context so the registry is dropped before the write lock is acquired
        // in `append_events()`
        {
            let mut registry = self.registry.write().await;
            for key in range {
                registry.remove(&key);
            }
        }

        self.append_events(events).await
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>>,
    ) -> anyhow::Result<()> {
        let mut registry = self.registry.write().await;
        for event in events {
            let block_number = event
                .meta
                .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?
                .block_number;
            if let contracts::cow_amm_constant_product_factory::Event::Deployed(event) = event.data
            {
                registry.insert(block_number, event);
            }
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        Ok(self
            .registry
            .read()
            .await
            .last_key_value()
            .map(|(block, _)| *block)
            .unwrap_or(self.first_block))
    }
}
