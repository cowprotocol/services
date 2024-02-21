use {
    crate::{database::Postgres, on_settlement_event_updater::OnSettlementEventUpdater},
    anyhow::Result,
    ethrpc::current_block::RangeInclusive,
    shared::{event_handling::EventStoring, impl_event_retrieving},
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
    async fn last_event_block(&self) -> Result<u64> {
        let mut con = self.db.pool.acquire().await?;
        crate::database::events::last_event_block(&mut con).await
    }

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

        self.settlement_updater.schedule_update();
        Ok(())
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
    ) -> Result<()> {
        let mut transaction = self.db.pool.begin().await?;
        crate::database::events::append_events(&mut transaction, events).await?;
        transaction.commit().await?;

        self.settlement_updater.schedule_update();
        Ok(())
    }
}
