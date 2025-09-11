use {
    crate::{database::Postgres, domain::settlement},
    anyhow::Result,
    ethcontract::{H256, contract::AllEventsBuilder},
    ethrpc::block_stream::RangeInclusive,
    hex_literal::hex,
    shared::event_handling::{EventRetrieving, EventStoring},
};

const SETTLEMENT: H256 = H256(hex!(
    "40338ce1a7c49204f0099533b1e9a7ee0a3d261f84974ab7af36105b8c4e9db4"
));
const TRADE: H256 = H256(hex!(
    "a07a543ab8a018198e99ca0184c93fe9050a79400a0a723441f84de1d972cc17"
));
const ORDER_INVALIDATED: H256 = H256(hex!(
    "875b6cb035bbd4ac6500fabc6d1e4ca5bdc58a3e2b424ccb5c24cdbebeb009a9"
));
const PRE_SIGNATURE: H256 = H256(hex!(
    "01bf7c8b0ca55deecbea89d7e58295b7ffbf685fd0d96801034ba8c6ffe1c68d"
));

static RELEVANT_EVENTS: &[H256] = &[SETTLEMENT, TRADE, ORDER_INVALIDATED, PRE_SIGNATURE];

pub struct GPv2SettlementContract(pub contracts::GPv2Settlement);

impl EventRetrieving for GPv2SettlementContract {
    type Event = contracts::gpv2_settlement::Event;

    fn get_events(
        &self,
    ) -> ethcontract::contract::AllEventsBuilder<ethcontract::dyns::DynTransport, Self::Event> {
        let mut events =
            AllEventsBuilder::new(self.0.raw_instance().web3().clone(), self.0.address(), None);
        events.filter = events.filter.topic0(RELEVANT_EVENTS.to_vec().into());
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
