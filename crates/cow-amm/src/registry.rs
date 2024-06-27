use {
    crate::{factories::Deployment, Amm},
    anyhow::Context,
    contracts::CowAmmLegacyHelper,
    ethcontract::Address,
    ethrpc::{
        current_block::{BlockRetrieving, CurrentBlockStream, RangeInclusive},
        Web3,
    },
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
    /// Store indexed data associated to the indexed events type id.
    /// That type erasure allows us to index multiple concrete contracts
    /// in a single Registry to make for a nicer user facing API.
    storage: Arc<RwLock<HashMap<TypeId, Storage>>>,
    web3: Web3,
    current_block_stream: CurrentBlockStream,
}

impl Registry {
    pub fn new(web3: Web3, current_block_stream: CurrentBlockStream) -> Self {
        Self {
            storage: Default::default(),
            web3,
            current_block_stream,
        }
    }

    pub async fn add_listener<C>(&self, first_block: u64, contract: C, helper_contract: Address)
    where
        C: EventRetrieving + Send + Sync + 'static,
        <C as EventRetrieving>::Event: Deployment,
    {
        let type_id = TypeId::of::<C::Event>();
        let helper = CowAmmLegacyHelper::at(&self.web3, helper_contract);
        let storage = Storage {
            cow_amms: Default::default(),
            first_block,
            helper,
        };

        self.storage.write().await.insert(type_id, storage);

        self.spawn_event_updater(
            Arc::new(self.web3.clone()),
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
    pub async fn cow_amms(&self) -> Vec<Arc<Amm>> {
        let cow_amms = self.storage.read().await;
        cow_amms
            .values()
            .flat_map(|storage| storage.cow_amms.values().flatten().cloned())
            .collect::<Vec<_>>()
    }
}

/// Stores CoW AMMs indexes for the associated factory contract.
struct Storage {
    /// Stores which AMMs were deployed on which block.
    cow_amms: BTreeMap<u64, Vec<Arc<Amm>>>,
    /// The block in which the associated factory contract was created.
    /// This is the block from which indexing should start.
    first_block: u64,
    /// Helper contract to query required data from the cow amm.
    helper: CowAmmLegacyHelper,
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
            let type_id = TypeId::of::<E>();
            let lock = &mut *self.storage.write().await;
            let storage = lock
                .get_mut(&type_id)
                .context("cow amm storage missing for factory")?;

            // Remove the Cow AMM events in the given range
            for key in *range.start()..=*range.end() {
                storage.cow_amms.remove(&key);
            }
        }

        // Apply all the new events
        self.append_events(events).await
    }

    /// Apply all the events to the given CoW AMM registry and update the
    /// internal registry
    async fn append_events(&mut self, events: Vec<ethcontract::Event<E>>) -> anyhow::Result<()> {
        let type_id = TypeId::of::<E>();
        let lock = &mut *self.storage.write().await;
        let storage = lock
            .get_mut(&type_id)
            .context("cow amm storage missing for factory")?;

        for event in events {
            let meta = event
                .meta
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?;
            let block_number = meta.block_number;
            if let Some(cow_amm) = event.data.deployed_amm(&storage.helper).await? {
                tracing::debug!(?cow_amm, "indexed new cow amm");
                storage
                    .cow_amms
                    .entry(block_number)
                    .or_default()
                    .push(Arc::new(cow_amm));
            }
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let type_id = TypeId::of::<E>();

        let lock = self.storage.read().await;
        let storage = lock
            .get(&type_id)
            .context("cow amm storage missing for factory")?;

        let last_block = storage
            .cow_amms
            .last_key_value()
            .map(|(block, _amms)| *block)
            .unwrap_or(storage.first_block);
        Ok(last_block)
    }
}
