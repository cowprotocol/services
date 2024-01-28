use {
    crate::domain,
    anyhow::{Context, Result},
    ethrpc::current_block::RangeInclusive,
    shared::{event_handling::EventStoring, impl_event_retrieving},
};

impl_event_retrieving! {
    pub GPv2SettlementContract for contracts::gpv2_settlement
}

pub struct Indexer {
    events: domain::Events,
}

impl Indexer {
    pub fn new(events: domain::Events) -> Self {
        Self { events }
    }
}

#[async_trait::async_trait]
impl EventStoring<contracts::gpv2_settlement::Event> for Indexer {
    async fn last_event_block(&self) -> Result<u64> {
        self.events
            .latest_block()
            .await
            .context("Error fetching latest settlement event block")
    }

    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        self.events
            .replace(events, range)
            .await
            .context("Error replacing settlement events")
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
    ) -> Result<()> {
        self.events
            .append(events)
            .await
            .context("Error appending settlement events")
    }
}
