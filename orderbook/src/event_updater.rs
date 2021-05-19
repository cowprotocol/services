use crate::database::Database;
use anyhow::{Context, Result};
use contracts::{g_pv_2_settlement::Event as ContractEvent, GPv2Settlement};
use ethcontract::contract::AllEventsBuilder;
use ethcontract::{dyns::DynTransport, Event};
use shared::event_handling::{BlockNumber, EventHandler, EventRetrieving, EventStoring};
use std::ops::{Deref, DerefMut, RangeInclusive};
use web3::Web3;

pub struct EventUpdater(EventHandler<GPv2SettlementContract, Database>);

impl Deref for EventUpdater {
    type Target = EventHandler<GPv2SettlementContract, Database>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EventUpdater {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait::async_trait]
impl EventStoring<ContractEvent> for Database {
    async fn replace_events(
        &self,
        events: Vec<Event<ContractEvent>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()> {
        let db_events = self
            .contract_to_db_events(events)
            .context("failed to get event")?;
        tracing::debug!(
            "replacing {} events from block number {}",
            db_events.len(),
            range.start().to_u64()
        );
        self.replace_events(range.start().to_u64(), db_events)
            .await
            .context("failed to replace trades")?;
        Ok(())
    }

    async fn append_events(&self, events: Vec<Event<ContractEvent>>) -> Result<()> {
        let db_events = self
            .contract_to_db_events(events)
            .context("failed to get event")?;
        tracing::debug!("inserting {} new events", db_events.len());
        self.insert_events(db_events)
            .await
            .context("failed to insert trades")?;
        Ok(())
    }

    async fn last_event_block(&self) -> Result<u64> {
        self.block_number_of_most_recent_event().await
    }
}

pub struct GPv2SettlementContract(GPv2Settlement);

impl EventRetrieving for GPv2SettlementContract {
    type Event = ContractEvent;
    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event> {
        self.0.all_events()
    }

    fn web3(&self) -> Web3<DynTransport> {
        self.0.raw_instance().web3()
    }
}

impl EventUpdater {
    pub fn new(contract: GPv2Settlement, db: Database, start_sync_at_block: Option<u64>) -> Self {
        Self(EventHandler::new(
            GPv2SettlementContract(contract),
            db,
            start_sync_at_block,
        ))
    }
}
