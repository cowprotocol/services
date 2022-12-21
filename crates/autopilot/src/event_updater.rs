use anyhow::Result;
use contracts::gpv2_settlement;
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

impl<Database, W> EventUpdater<Database, W>
where
    Database: EventStoring<<W as EventRetrieving>::Event>,
    W: EventRetrieving + Send + Sync,
{
    /// Creates a new event updater.
    ///
    /// If a start sync block is specified, it will always resync events from this poing on creation,
    /// regardless of them being already available in the database.
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

    /// Creates a new event updater.
    ///
    /// Similar to [`Self::new()`]: the main different is that the required starting sync point
    /// specifies a value before which events should not be indexed. If there are no events
    /// available in the database (or only older events) it starts indexing from this point. If
    /// there are more recent events available, then the sync start is ignored.
    pub async fn new_skip_blocks_before(
        contract: W,
        db: Database,
        block_retriever: Arc<dyn BlockRetrieving>,
        start_sync_at_block: BlockNumberHash,
    ) -> Result<Self> {
        Ok(Self(Mutex::new(
            EventHandler::new_skip_blocks_before(
                block_retriever,
                contract,
                db,
                start_sync_at_block,
            )
            .await?,
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

    fn name(&self) -> &str {
        "EventUpdater"
    }
}
