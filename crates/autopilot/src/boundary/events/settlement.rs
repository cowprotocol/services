use {
    crate::database::Postgres,
    alloy::{
        primitives::Address,
        rpc::types::{Filter, Log},
    },
    anyhow::Result,
    contracts::alloy::GPv2Settlement::GPv2Settlement::GPv2SettlementEvents,
    ethrpc::{AlloyProvider, block_stream::RangeInclusive},
    shared::event_handling::{AlloyEventRetrieving, EventStoring},
};

pub struct GPv2SettlementContract {
    provider: AlloyProvider,
    address: Address,
}

impl GPv2SettlementContract {
    pub fn new(provider: AlloyProvider, address: Address) -> Self {
        Self { provider, address }
    }
}

impl AlloyEventRetrieving for GPv2SettlementContract {
    type Event = GPv2SettlementEvents;

    fn filter(&self) -> alloy::rpc::types::Filter {
        Filter::new().address(self.address)
    }

    fn provider(&self) -> &alloy::providers::DynProvider {
        &self.provider
    }
}

pub struct Indexer {
    db: Postgres,
    start_indexing_block: u64,
}

impl Indexer {
    pub fn new(db: Postgres, start_indexing_block: u64) -> Self {
        Self {
            db,
            start_indexing_block,
        }
    }
}

/// This name is used to store the latest indexed block in the db.
pub(crate) const INDEX_NAME: &str = "settlements";

#[async_trait::async_trait]
impl EventStoring<(GPv2SettlementEvents, Log)> for Indexer {
    async fn last_event_block(&self) -> Result<u64> {
        super::read_last_block_from_db(&self.db.pool, INDEX_NAME)
            .await
            .map(|last_block| last_block.max(self.start_indexing_block))
    }

    async fn persist_last_indexed_block(&mut self, latest_block: u64) -> Result<()> {
        super::write_last_block_to_db(&self.db.pool, latest_block, INDEX_NAME).await
    }

    async fn replace_events(
        &mut self,
        events: Vec<(GPv2SettlementEvents, Log)>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        let mut transaction = self.db.pool.begin().await?;
        let from_block = *range.start();
        crate::database::events::replace_events(&mut transaction, events, from_block).await?;
        database::settlements::delete(&mut transaction, from_block).await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn append_events(&mut self, events: Vec<(GPv2SettlementEvents, Log)>) -> Result<()> {
        let mut transaction = self.db.pool.begin().await?;
        crate::database::events::append_events(&mut transaction, events).await?;
        transaction.commit().await?;
        Ok(())
    }
}
