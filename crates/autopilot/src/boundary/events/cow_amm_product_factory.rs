use {
    contracts::cow_amm_constant_product_factory::Event,
    ethrpc::current_block::RangeInclusive,
    shared::{event_handling::EventStoring, impl_event_retrieving},
    std::collections::BTreeMap,
};

impl_event_retrieving! {
    pub CowAmmConstantProductFactoryContract for contracts::cow_amm_constant_product_factory
}

#[derive(Default)]
/// CoW AMM indexer which stores events in-memory.
pub struct Indexer {
    /// Registry with the format: (block number, event)
    registry: BTreeMap<u64, contracts::cow_amm_constant_product_factory::event_data::Deployed>,
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
        let keys_to_remove: Vec<u64> = self
            .registry
            .range(range)
            .map(|(block, _)| *block)
            .collect();

        for key in keys_to_remove {
            self.registry.remove(&key);
        }

        self.append_events(events).await
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>>,
    ) -> anyhow::Result<()> {
        events.into_iter().try_for_each(|event| {
            let block_number = event
                .meta
                .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?
                .block_number;
            match event.data {
                Event::Deployed(event) => {
                    self.registry.insert(block_number, event);
                }
                _ => {}
            }
            Ok::<_, anyhow::Error>(())
        })?;
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        self.registry
            .last_key_value()
            .map(|(block, _)| *block)
            .ok_or_else(|| anyhow::anyhow!("No events found"))
    }
}
