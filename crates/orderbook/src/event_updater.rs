use anyhow::Result;
use contracts::{
    gpv2_settlement::{self, Event as ContractEvent},
    GPv2Settlement,
};
use ethcontract::dyns::DynWeb3;
use shared::{
    event_handling::{EventHandler, EventStoring},
    impl_event_retrieving,
    maintenance::Maintaining,
};
use tokio::sync::Mutex;

/// A trait for abstracting event updater methods.
#[async_trait::async_trait]
pub trait EventUpdating: Send + Sync + 'static {
    /// Returns the last handled block number. Events are guaranteed to be
    /// processed up until this block.
    async fn last_handled_block(&self) -> Option<u64>;
}

pub struct EventUpdater<Database: EventStoring<ContractEvent>>(
    Mutex<EventHandler<DynWeb3, GPv2SettlementContract, Database>>,
);

impl_event_retrieving! {
    pub GPv2SettlementContract for gpv2_settlement
}

impl<Database> EventUpdater<Database>
where
    Database: EventStoring<ContractEvent>,
{
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
impl<Database> EventUpdating for EventUpdater<Database>
where
    Database: EventStoring<ContractEvent> + Send + Sync + 'static,
{
    async fn last_handled_block(&self) -> Option<u64> {
        self.0.lock().await.last_handled_block()
    }
}

#[async_trait::async_trait]
impl<Database> Maintaining for EventUpdater<Database>
where
    Database: EventStoring<ContractEvent>,
{
    async fn run_maintenance(&self) -> Result<()> {
        self.0.run_maintenance().await
    }
}
