use {
    crate::{ContractHandler, CowAmm},
    ethcontract::common::DeploymentInformation,
    ethrpc::current_block::RangeInclusive,
    shared::event_handling::EventStoring,
    std::{collections::BTreeMap, sync::Arc},
    tokio::sync::RwLock,
};

/// Registry with the format: (block number, cow_amm)
pub type CowAmmRegistry = BTreeMap<u64, Arc<dyn CowAmm>>;

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone)]
pub struct Indexer<E> {
    contract_handler: Arc<dyn ContractHandler<E>>,
    cow_amms: Arc<RwLock<CowAmmRegistry>>,
    first_block: u64,
}

impl<E> Indexer<E> {
    pub async fn new(contract_handler: Arc<dyn ContractHandler<E>>) -> Self {
        let first_block = Self::first_block(contract_handler.clone()).await;
        Self {
            cow_amms: Arc::new(RwLock::new(BTreeMap::new())),
            first_block,
            contract_handler,
        }
    }

    async fn first_block(contract_handler: Arc<dyn ContractHandler<E>>) -> u64 {
        if let Some(DeploymentInformation::BlockNumber(block)) =
            contract_handler.deployment_information()
        {
            block
        } else {
            0
        }
    }

    /// Returns all CoW AMMs that are currently enabled (i.e. able to trade).
    pub async fn cow_amms(&self) -> Vec<Arc<dyn crate::CowAmm>> {
        let cow_amms = self.cow_amms.read().await;
        cow_amms.values().cloned().collect::<Vec<_>>()
    }
}

#[async_trait::async_trait]
impl<E: Send + Sync> EventStoring<E> for Indexer<E> {
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
                self.contract_handler
                    .apply_event(block_number, &event.data, cow_amms)
                    .await?;
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
