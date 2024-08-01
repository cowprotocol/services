use {
    crate::Amm,
    contracts::{cow_amm_legacy_helper::Event as CowAmmEvent, CowAmmLegacyHelper},
    ethcontract::Address,
    ethrpc::current_block::RangeInclusive,
    shared::event_handling::EventStoring,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
    tokio::sync::RwLock,
};

#[derive(Clone, Debug)]
pub(crate) struct Storage(Arc<Inner>);

impl Storage {
    pub(crate) fn new(deployment_block: u64, helper: CowAmmLegacyHelper) -> Self {
        Self(Arc::new(Inner {
            cache: Default::default(),
            // make sure to start 1 block **before** the deployment to get all the events
            start_of_index: deployment_block - 1,
            helper,
        }))
    }

    pub(crate) async fn cow_amms(&self) -> Vec<Arc<Amm>> {
        let lock = self.0.cache.read().await;
        lock.values()
            .flat_map(|amms| amms.iter().cloned())
            .collect()
    }

    pub(crate) async fn remove_amms(&self, amm_addresses: &HashSet<Address>) {
        let mut lock = self.0.cache.write().await;
        for (_, amms) in lock.iter_mut() {
            amms.retain(|amm| !amm_addresses.contains(amm.address()))
        }
    }
}

#[derive(Debug)]
struct Inner {
    /// Store indexed data associated to the indexed events type id.
    /// That type erasure allows us to index multiple concrete contracts
    /// in a single Registry to make for a nicer user facing API.
    cache: RwLock<BTreeMap<u64, Vec<Arc<Amm>>>>,
    /// The earliest block where indexing the contract makes sense.
    /// The contract did not emit any events before this block.
    start_of_index: u64,
    /// Helper contract to query required data from the cow amm.
    helper: CowAmmLegacyHelper,
}

#[async_trait::async_trait]
impl EventStoring<CowAmmEvent> for Storage {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<CowAmmEvent>>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        // Context to drop the write lock before calling `append_events()`
        {
            let cache = &mut *self.0.cache.write().await;

            // Remove the Cow AMM events in the given range
            for key in *range.start()..=*range.end() {
                cache.remove(&key);
            }
        }

        // Apply all the new events
        self.append_events(events).await
    }

    /// Apply all the events to the given CoW AMM registry and update the
    /// internal registry
    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<CowAmmEvent>>,
    ) -> anyhow::Result<()> {
        let cache = &mut *self.0.cache.write().await;

        for event in events {
            let meta = event
                .meta
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?;
            let CowAmmEvent::CowammpoolCreated(cow_amm) = event.data;
            let cow_amm = cow_amm.amm;

            cache
                .entry(meta.block_number)
                .or_default()
                .push(Arc::new(Amm::new(cow_amm, &self.0.helper).await?));
            tracing::info!(?cow_amm, "indexed new cow amm");
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let cache = self.0.cache.read().await;

        let last_block = cache
            .last_key_value()
            .map(|(block, _amms)| *block)
            .unwrap_or(self.0.start_of_index);
        Ok(last_block)
    }
}
