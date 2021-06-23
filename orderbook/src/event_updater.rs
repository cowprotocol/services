use crate::database::Postgres;
use anyhow::Result;
use contracts::{
    gpv2_settlement::{self},
    GPv2Settlement,
};
use ethcontract::dyns::DynWeb3;
use shared::{event_handling::EventHandler, impl_event_retrieving, maintenance::Maintaining};
use tokio::sync::Mutex;

pub struct EventUpdater(Mutex<EventHandler<DynWeb3, GPv2SettlementContract, Postgres>>);

impl_event_retrieving! {
    pub GPv2SettlementContract for gpv2_settlement
}

impl EventUpdater {
    pub fn new(contract: GPv2Settlement, db: Postgres, start_sync_at_block: Option<u64>) -> Self {
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
