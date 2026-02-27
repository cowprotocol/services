use {
    crate::{Amm, cache::Storage, factory::Factory, maintainers::EmptyPoolRemoval},
    alloy::primitives::Address,
    contracts::cow_amm::CowAmmLegacyHelper,
    ethrpc::{
        AlloyProvider,
        block_stream::{BlockRetriever, CurrentBlockWatcher},
    },
    shared::{
        event_handling::EventHandler,
        maintenance::{Maintaining, ServiceMaintenance},
    },
    sqlx::PgPool,
    std::sync::Arc,
    tokio::sync::{Mutex, RwLock},
    tracing::instrument,
};

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone)]
pub struct Registry {
    block_retriever: Arc<BlockRetriever>,
    storage: Arc<RwLock<Vec<Storage>>>,
    maintenance_tasks: Vec<Arc<dyn Maintaining>>,
}

impl Registry {
    pub fn new(block_retriever: Arc<BlockRetriever>) -> Self {
        Self {
            storage: Default::default(),
            block_retriever,
            maintenance_tasks: vec![],
        }
    }

    fn provider(&self) -> &AlloyProvider {
        &self.block_retriever.provider
    }

    /// Registers a new listener to detect CoW AMMs deployed by `factory`.
    /// Interfacing with the CoW AMM happens via the
    /// [`contracts::CowAmmLegacyHelper`] deployed at `helper_contract`.
    /// To actually start indexing these pools call `spawn_maintenance_tasks()`.
    #[instrument(skip_all)]
    pub async fn add_listener(
        &mut self,
        deployment_block: u64,
        factory: Address,
        helper_contract: Address,
        db: PgPool,
    ) {
        let storage = Storage::new(
            deployment_block,
            CowAmmLegacyHelper::Instance::new(helper_contract, self.provider().clone()),
            factory,
            db,
        )
        .await;

        self.storage.write().await.push(storage.clone());

        let indexer = Factory {
            provider: self.provider().clone(),
            address: factory,
        };
        let event_handler = EventHandler::new(self.block_retriever.clone(), indexer, storage, None);
        let token_balance_maintainer =
            EmptyPoolRemoval::new(self.storage.clone(), self.provider().clone());

        self.maintenance_tasks
            .push(Arc::new(Mutex::new(event_handler)));
        self.maintenance_tasks
            .push(Arc::new(token_balance_maintainer));
    }

    /// Returns all the deployed CoW AMMs
    #[instrument(skip_all)]
    pub async fn amms(&self) -> Vec<Arc<Amm>> {
        let mut result = vec![];
        let lock = self.storage.read().await;
        for cache in &*lock {
            result.extend(cache.cow_amms().await);
        }
        result
    }

    pub fn spawn_maintenance_task(&self, block_stream: CurrentBlockWatcher) {
        let maintenance = ServiceMaintenance::new(self.maintenance_tasks.clone());
        tokio::task::spawn(maintenance.run_maintenance_on_new_block(block_stream));
    }

    pub fn maintenance_tasks(&self) -> &Vec<Arc<dyn Maintaining>> {
        &self.maintenance_tasks
    }
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("block_retriever", &self.block_retriever)
            .field("storage", &self.storage)
            .finish()
    }
}
