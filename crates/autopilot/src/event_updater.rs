use anyhow::Result;
use contracts::{cowswap_onchain_orders, gpv2_settlement};
use ethcontract::dyns::DynWeb3;
use shared::{
    event_handling::{BlockNumberHash, EventHandler, EventRetrieving, EventStoring},
    impl_event_retrieving,
    maintenance::Maintaining,
    Web3,
};
use tokio::sync::Mutex;

pub struct EventUpdater<
    Database: EventStoring<<W as EventRetrieving>::Event>,
    W: EventRetrieving + Send + Sync,
>(Mutex<EventHandler<DynWeb3, W, Database>>);

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
        web3: Web3,
        start_sync_at_block: Option<BlockNumberHash>,
    ) -> Self {
        Self(Mutex::new(EventHandler::new(
            web3,
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
