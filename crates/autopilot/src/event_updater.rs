use anyhow::Result;
use contracts::{cowswap_onchain_orders, gpv2_settlement};
use shared::{
    current_block::{BlockNumberHash, BlockRetrieving},
    event_handling::{EventHandler, EventRetrieving, EventStoring},
    impl_event_retrieving,
    maintenance::Maintaining,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct EventUpdater<
    Database: EventStoring<<W as EventRetrieving>::Event>,
    W: EventRetrieving + Send + Sync,
>(Mutex<EventHandler<W, Database>>);

impl_event_retrieving! {
    pub GPv2SettlementContract for gpv2_settlement
}

impl_event_retrieving! {
    pub CoWSwapOnchainOrdersContract for cowswap_onchain_orders
}

impl<Database, W> EventUpdater<Database, W>
where
    Database: EventStoring<<W as EventRetrieving>::Event>,
    W: EventRetrieving + Send + Sync,
{
    pub fn new(
        contract: W,
        db: Database,
        block_retriever: Arc<dyn BlockRetrieving>,
        start_sync_at_block: Option<BlockNumberHash>,
    ) -> Self {
        Self(Mutex::new(EventHandler::new(
            block_retriever,
            contract,
            db,
            start_sync_at_block,
        )))
    }
}

#[async_trait::async_trait]
impl<Database, W> Maintaining for EventUpdater<Database, W>
where
    Database: EventStoring<<W>::Event>,
    W: EventRetrieving + Send + Sync,
{
    async fn run_maintenance(&self) -> Result<()> {
        self.0.run_maintenance().await
    }
}
