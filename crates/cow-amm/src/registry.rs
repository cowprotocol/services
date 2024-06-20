use {
    crate::{event_updater::EventUpdater, ContractHandler, CowAmm},
    ethcontract::common::DeploymentInformation,
    ethrpc::current_block::{BlockRetrieving, CurrentBlockStream, RangeInclusive},
    shared::event_handling::{EventRetrieving, EventStoring},
    std::{collections::BTreeMap, sync::Arc},
    tokio::sync::RwLock,
};

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone)]
pub struct Registry {
    cow_amms: Arc<RwLock<BTreeMap<u64, Arc<dyn CowAmm>>>>,
    first_block: u64,
}

impl Registry {
    pub async fn build<W>(
        block_retriever: Arc<dyn BlockRetrieving>,
        contract: W,
        current_block_stream: CurrentBlockStream,
        deployment_information: Option<DeploymentInformation>,
    ) -> Self
    where
        W: EventRetrieving + Send + Sync + 'static,
        <W as EventRetrieving>::Event: ContractHandler,
        <W as EventRetrieving>::Event: Clone,
    {
        let first_block =
            if let Some(DeploymentInformation::BlockNumber(block)) = deployment_information {
                block
            } else {
                0
            };
        let indexer = Self {
            cow_amms: Arc::new(RwLock::new(BTreeMap::new())),
            first_block,
        };

        EventUpdater::build(
            block_retriever,
            indexer.clone(),
            contract,
            current_block_stream,
        )
        .await;

        indexer
    }

    /// Returns all CoW AMMs that are currently enabled (i.e. able to trade).
    pub async fn cow_amms(&self) -> Vec<Arc<dyn crate::CowAmm>> {
        let cow_amms = self.cow_amms.read().await;
        cow_amms.values().cloned().collect::<Vec<_>>()
    }
}

#[async_trait::async_trait]
impl<E: ContractHandler + Clone> EventStoring<E> for Registry
where
    E: ContractHandler + 'static,
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
                if let Some(cow_amm) = event.data.apply_event().await? {
                    cow_amms.insert(block_number, cow_amm);
                }
            }
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        Ok(self
            .cow_amms
            .read()
            .await
            .last_key_value()
            .map(|(block, _)| *block)
            .unwrap_or(self.first_block))
    }
}
