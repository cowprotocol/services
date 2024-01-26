use {
    crate::database::Postgres,
    anyhow::Result,
    ethrpc::current_block::RangeInclusive,
    shared::{event_handling::EventStoring, impl_event_retrieving},
};

impl_event_retrieving! {
    pub GPv2SettlementContract for contracts::gpv2_settlement
}

pub struct Indexer {
    db: Postgres,
}

impl Indexer {
    pub fn new(db: Postgres) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl EventStoring<contracts::gpv2_settlement::Event> for Indexer {
    async fn last_event_block(&self) -> Result<u64> {
        let store: &dyn EventStoring<contracts::gpv2_settlement::Event> = &self.db;
        store.last_event_block().await
    }

    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        self.db.replace_events(events, range).await
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
    ) -> Result<()> {
        self.db.append_events(events).await
    }
}
