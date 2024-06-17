use {
    crate::{
        cow_amm::CowAmm,
        cow_amm_constant_product_factory::CowAmmConstantProductFactoryHandler,
    },
    contracts::cow_amm_constant_product_factory,
    ethcontract::{common::DeploymentInformation, dyns::DynWeb3, Address},
    ethrpc::current_block::RangeInclusive,
    shared::event_handling::EventStoring,
    std::{
        collections::{BTreeMap, HashMap},
        sync::Arc,
    },
    tokio::sync::RwLock,
};

pub type CowAmmRegistry = HashMap<Address, CowAmm>;
/// Registry with the format: (block number, (log index, event))
pub type EventsRegistry = BTreeMap<u64, cow_amm_constant_product_factory::Event>;

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone)]
pub struct Indexer {
    cow_amms: Arc<RwLock<CowAmmRegistry>>,
    events_registry: Arc<RwLock<EventsRegistry>>,
    first_block: u64,
}

impl Indexer {
    pub async fn new(web3: &DynWeb3, cow_amm_factory_address: Option<&Address>) -> Self {
        let cow_amm_constant_product_factory =
            if let Some(cow_amm_factory_address) = cow_amm_factory_address {
                contracts::CowAmmConstantProductFactory::at(web3, *cow_amm_factory_address)
            } else {
                contracts::CowAmmConstantProductFactory::deployed(web3)
                    .await
                    .expect("Failed to find deployed CowAmmConstantProductFactory")
            };
        let first_block = match cow_amm_constant_product_factory
            .deployment_information()
            .expect("Failed to get deployment information")
        {
            DeploymentInformation::BlockNumber(block) => block,
            _ => panic!("Expected block number"),
        };
        Self {
            cow_amms: Arc::new(RwLock::new(HashMap::new())),
            events_registry: Arc::new(RwLock::new(BTreeMap::new())),
            first_block,
        }
    }

    /// Returns all CoW AMMs that are currently enabled (i.e. able to trade).
    pub async fn enabled_cow_amms(&self) -> Vec<impl crate::CowAmm> {
        let cow_amms = self.cow_amms.read().await;
        cow_amms
            .values()
            .filter(|cow_amm| cow_amm.is_enabled())
            .cloned()
            .collect::<Vec<_>>()
    }
}

#[async_trait::async_trait]
impl EventStoring<cow_amm_constant_product_factory::Event> for Indexer {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<cow_amm_constant_product_factory::Event>>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        // Context to drop the write lock before calling `reapply_events()`
        {
            let range = *range.start()..=*range.end();
            let events_registry = &mut *self.events_registry.write().await;

            // Remove the CowAmmConstantProductFactoryEvent events in the given range
            for key in range {
                events_registry.remove(&key);
            }
        }

        // Revert the CoW AMM status after removing the events in the specified
        // range. Since we cannot know what is the CoW AMM status before an
        // event is reverted, in order to guarantee the CoW AMM status is
        // correct, we need to reapply all the events and substitute
        // the current CoW AMM in memory after the reapplication.
        // It returns the new list of CoW AMMs after all the events in the
        // registry have been applied
        // Context to drop the write lock before calling `reapply_events()`
        {
            let mut cow_amms = HashMap::new();
            for event in &events {
                CowAmmConstantProductFactoryHandler::apply_event(&event.data, &mut cow_amms)
                    .await?;
            }
            *self.cow_amms.write().await = cow_amms;
        }

        // Apply the new events
        self.append_events(events).await
    }

    /// Apply all the events to the given CoW AMM registry and update the
    /// internal registry
    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<cow_amm_constant_product_factory::Event>>,
    ) -> anyhow::Result<()> {
        let cow_amms = &mut *self.cow_amms.write().await;
        let events_registry = &mut *self.events_registry.write().await;
        for event in events {
            let meta = event
                .meta
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?;
            let block_number = meta.block_number;

            CowAmmConstantProductFactoryHandler::apply_event(&event.data, cow_amms).await?;

            events_registry.insert(block_number, event.data);
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        Ok(self
            .events_registry
            .read()
            .await
            .iter()
            .last()
            .map(|(block, _)| *block)
            .unwrap_or(self.first_block))
    }
}
