use crate::database::Database;
use anyhow::{Context, Result};
use contracts::{
    gpv2_settlement::{self, Event as ContractEvent},
    GPv2Settlement,
};
use ethcontract::{dyns::DynWeb3, Event};
use shared::{
    event_handling::{BlockNumber, EventHandler, EventStoring},
    impl_event_retrieving,
    maintenance::Maintaining,
};
use std::ops::RangeInclusive;
use tokio::sync::Mutex;

pub struct EventUpdater(Mutex<EventHandler<DynWeb3, GPv2SettlementContract, Database>>);

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

impl_event_retrieving! {
    pub GPv2SettlementContract for gpv2_settlement
}

impl EventUpdater {
    pub fn new(contract: GPv2Settlement, db: Database, start_sync_at_block: Option<u64>) -> Self {
        Self(Mutex::new(EventHandler::new(
            contract.raw_instance().web3(),
            GPv2SettlementContract(contract),
            db,
            start_sync_at_block,
        )))
    }
}

#[async_trait::async_trait]
impl Maintaining for EventUpdater {
    async fn run_maintenance(&self) -> Result<()> {
        self.0.run_maintenance().await
    }
}
