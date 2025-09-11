use {
    crate::{database::Postgres, domain::settlement},
    anyhow::Result,
    ethcontract::contract::AllEventsBuilder,
    ethrpc::block_stream::RangeInclusive,
    shared::event_handling::{EventRetrieving, EventStoring},
};

pub struct GPv2SettlementContract(pub contracts::GPv2Settlement);

impl EventRetrieving for GPv2SettlementContract {
    type Event = contracts::gpv2_settlement::Event;

    fn get_events(
        &self,
    ) -> ethcontract::contract::AllEventsBuilder<ethcontract::dyns::DynTransport, Self::Event> {
        let mut events =
            AllEventsBuilder::new(self.0.raw_instance().web3().clone(), self.0.address(), None);
        events.filter = events.filter.address(vec![self.0.address()]).topic0(
            vec![
                contracts::gpv2_settlement::event_data::Settlement::signature(),
                contracts::gpv2_settlement::event_data::Trade::signature(),
                contracts::gpv2_settlement::event_data::OrderInvalidated::signature(),
                contracts::gpv2_settlement::event_data::PreSignature::signature(),
                // we don't index Interactions because we don't care about them
            ]
            .into(),
        );
        events
    }
}

pub struct Indexer {
    db: Postgres,
    start_index: u64,
    settlement_observer: settlement::Observer,
}

impl Indexer {
    pub fn new(db: Postgres, settlement_observer: settlement::Observer, start_index: u64) -> Self {
        Self {
            db,
            settlement_observer,
            start_index,
        }
    }
}

/// This name is used to store the latest indexed block in the db.
pub(crate) const INDEX_NAME: &str = "settlements";

#[async_trait::async_trait]
impl EventStoring<contracts::gpv2_settlement::Event> for Indexer {
    async fn last_event_block(&self) -> Result<u64> {
        super::read_last_block_from_db(&self.db.pool, INDEX_NAME)
            .await
            .map(|last_block| last_block.max(self.start_index))
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
