use {
    crate::{CowAmm, Deployment},
    ethrpc::current_block::{BlockRetrieving, CurrentBlockStream, RangeInclusive},
    shared::{
        event_handling::{EventHandler, EventRetrieving, EventStoring},
        maintenance::{Maintaining, ServiceMaintenance},
    },
    std::{
        any::TypeId,
        collections::{BTreeMap, HashMap},
        sync::Arc,
    },
    tokio::sync::{Mutex, RwLock},
};

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone)]
pub struct Registry {
    cow_amms: Arc<RwLock<BTreeMap<u64, Arc<dyn CowAmm>>>>,
    first_blocks: Arc<RwLock<HashMap<TypeId, u64>>>,
    block_retriever: Arc<dyn BlockRetrieving>,
    current_block_stream: CurrentBlockStream,
}

impl Registry {
    pub fn new(
        block_retriever: Arc<dyn BlockRetrieving>,
        current_block_stream: CurrentBlockStream,
    ) -> Self {
        Self {
            cow_amms: Arc::new(RwLock::new(BTreeMap::new())),
            first_blocks: Arc::new(RwLock::new(HashMap::new())),
            block_retriever,
            current_block_stream,
        }
    }

    pub async fn add_listener<C>(&self, contract: C, first_block: u64)
    where
        C: EventRetrieving + Send + Sync + 'static,
        <C as EventRetrieving>::Event: Deployment,
    {
        {
            let first_blocks = &mut self.first_blocks.write().await;
            let type_id = TypeId::of::<C::Event>();
            first_blocks.insert(type_id, first_block);
        }

        self.spawn_event_updater(
            self.block_retriever.clone(),
            contract,
            self.current_block_stream.clone(),
        )
        .await;
    }

    async fn spawn_event_updater<C>(
        &self,
        block_retriever: Arc<dyn BlockRetrieving>,
        contract: C,
        current_block_stream: CurrentBlockStream,
    ) where
        C: EventRetrieving + Send + Sync + 'static,
        <C as EventRetrieving>::Event: Deployment,
    {
        let event_handler = EventHandler::new(block_retriever, contract, self.clone(), None);
        let event_handler: Vec<Arc<dyn Maintaining>> = vec![Arc::new(Mutex::new(event_handler))];
        let service_maintainer = ServiceMaintenance::new(event_handler);
        tokio::task::spawn(service_maintainer.run_maintenance_on_new_block(current_block_stream));
    }

    /// Returns all the deployed CoW AMMs
    pub async fn cow_amms(&self) -> Vec<Arc<dyn crate::CowAmm>> {
        let cow_amms = self.cow_amms.read().await;
        cow_amms.values().cloned().collect::<Vec<_>>()
    }
}

#[async_trait::async_trait]
impl<E: Deployment> EventStoring<E> for Registry
where
    E: Deployment + 'static,
{
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<E>>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        // Context to drop the write lock before calling `append_events()`
        {
            let range = *range.start()..=*range.end();
            let events_registry = &mut *self.cow_amms.write().await;

            // Remove the Cow AMM events in the given range
            for key in range {
                events_registry.remove(&key);
            }
        }

        // Apply all the new events
        self.append_events(events).await
    }

    /// Apply all the events to the given CoW AMM registry and update the
    /// internal registry
    async fn append_events(&mut self, events: Vec<ethcontract::Event<E>>) -> anyhow::Result<()> {
        {
            let cow_amms = &mut *self.cow_amms.write().await;
            for event in events {
                let meta = event
                    .meta
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?;
                let block_number = meta.block_number;
                if let Some(cow_amm) = event.data.deployed_amm().await {
                    cow_amms.insert(block_number, cow_amm);
                }
            }
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let type_id = TypeId::of::<E>();

        let first_block = self
            .first_blocks
            .read()
            .await
            .get(&type_id)
            .copied()
            .unwrap_or(0);

        Ok(self
            .cow_amms
            .read()
            .await
            .last_key_value()
            .map(|(block, _)| *block)
            .unwrap_or(first_block))
    }
}
