use {
    crate::{database::Postgres, on_settlement_event_updater::OnSettlementEventUpdater},
    anyhow::Result,
    ethrpc::block_stream::RangeInclusive,
    shared::{
        event_handling::{EventStoring, PgEventCounter},
        impl_event_retrieving,
    },
    sqlx::PgPool,
};

impl_event_retrieving! {
    pub GPv2SettlementContract for contracts::gpv2_settlement
}

pub struct Indexer {
    db: Postgres,
    settlement_updater: OnSettlementEventUpdater,
}

impl Indexer {
    pub fn new(db: Postgres, settlement_updater: OnSettlementEventUpdater) -> Self {
        Self {
            db,
            settlement_updater,
        }
    }
}

#[async_trait::async_trait]
impl EventStoring<contracts::gpv2_settlement::Event> for Indexer {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        let mut transaction = self.db.pool.begin().await?;
        let from_block = *range.start();
        crate::database::events::replace_events(&mut transaction, events, from_block).await?;
        OnSettlementEventUpdater::delete_observations(&mut transaction, from_block).await?;
        transaction.commit().await?;

        self.settlement_updater.update().await;
        Ok(())
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
    ) -> Result<()> {
        let mut transaction = self.db.pool.begin().await?;
        crate::database::events::append_events(&mut transaction, events).await?;
        transaction.commit().await?;

        self.settlement_updater.update().await;
        Ok(())
    }

    async fn last_event_block(&self) -> Result<u64> {
        PgEventCounter::last_event_block(self).await
    }

    async fn update_counter(&mut self, new_value: u64) -> Result<()> {
        PgEventCounter::update_counter(self, new_value).await
    }
}

#[async_trait::async_trait]
impl PgEventCounter<contracts::gpv2_settlement::Event> for Indexer {
    const INDEXER_NAME: &'static str = "gpv2_settlement_indexer";

    fn pg_pool(&self) -> &PgPool {
        &self.db.pool
    }
}
