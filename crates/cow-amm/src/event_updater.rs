use {
    ethrpc::current_block::{BlockRetrieving, CurrentBlockStream},
    shared::{
        event_handling::{EventHandler, EventRetrieving, EventStoring},
        maintenance::{Maintaining, ServiceMaintenance},
    },
    std::sync::Arc,
    tokio::sync::Mutex,
};

pub(crate) struct EventUpdater<
    Database: EventStoring<<W as EventRetrieving>::Event>,
    W: EventRetrieving + Send + Sync,
>(Mutex<EventHandler<W, Database>>);

impl<Indexer, W> EventUpdater<Indexer, W>
where
    Indexer: EventStoring<<W as EventRetrieving>::Event> + 'static,
    W: EventRetrieving + Send + Sync + 'static,
{
    pub(crate) async fn build(
        block_retriever: Arc<dyn BlockRetrieving>,
        indexer: Indexer,
        contract: W,
        current_block_stream: CurrentBlockStream,
    ) {
        let event_handler = EventHandler::new(block_retriever, contract, indexer, None);
        let event_handler: Vec<Arc<dyn Maintaining>> =
            vec![Arc::new(Self(Mutex::new(event_handler)))];
        let service_maintainer = ServiceMaintenance::new(event_handler);
        tokio::task::spawn(service_maintainer.run_maintenance_on_new_block(current_block_stream));
    }
}

#[async_trait::async_trait]
impl<Indexer, W> Maintaining for EventUpdater<Indexer, W>
where
    Indexer: EventStoring<<W>::Event>,
    W: EventRetrieving + Send + Sync,
{
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        self.0.run_maintenance().await
    }

    fn name(&self) -> &str {
        "CowAmmEventUpdater"
    }
}
