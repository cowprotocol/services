use {
    crate::{database::Postgres, domain::settlement},
    anyhow::Result,
    ethrpc::block_stream::RangeInclusive,
    shared::{event_handling::EventStoring, impl_event_retrieving},
};

impl_event_retrieving! {
    pub GPv2SettlementContract for contracts::gpv2_settlement
}

pub struct Indexer {
    db: Postgres,
    settlement_observer: settlement::Observer,
}

impl Indexer {
    pub fn new(db: Postgres, settlement_observer: settlement::Observer) -> Self {
        Self {
            db,
            settlement_observer,
        }
    }
}

/// This name is used to store the latest indexed block in the db.
const INDEX_NAME: &str = "settlements";

#[async_trait::async_trait]
impl EventStoring<contracts::gpv2_settlement::Event> for Indexer {
    async fn last_event_block(&self) -> Result<u64> {
        super::read_last_block_from_db(&self.db.pool, INDEX_NAME).await
    }

    async fn persist_last_indexed_block(&mut self, latest_block: u64) -> Result<()> {
        super::write_last_block_to_db(&self.db.pool, latest_block, INDEX_NAME).await
    }

    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        let mut transaction = self.db.pool.begin().await?;
        let from_block = *range.start();
        crate::database::events::replace_events(&mut transaction, events, from_block).await?;
        database::settlements::delete(&mut transaction, from_block).await?;
        transaction.commit().await?;

        self.settlement_observer.update().await;
        Ok(())
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
    ) -> Result<()> {
        let mut transaction = self.db.pool.begin().await?;
        crate::database::events::append_events(&mut transaction, events).await?;
        transaction.commit().await?;

        self.settlement_observer.update().await;
        Ok(())
    }
}
