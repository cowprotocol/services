use {
    crate::{cache::Storage, factory::Factory, maintainers::EmptyPoolRemoval, Amm},
    contracts::CowAmmLegacyHelper,
    ethcontract::Address,
    ethrpc::{block_stream::CurrentBlockWatcher, Web3},
    shared::{
        event_handling::EventHandler,
        maintenance::{Maintaining, ServiceMaintenance},
    },
    std::sync::Arc,
    tokio::sync::{Mutex, RwLock},
};

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone, Debug)]
pub struct Registry {
    web3: Web3,
    current_block_stream: CurrentBlockWatcher,
    storage: Arc<RwLock<Vec<Storage>>>,
}

impl Registry {
    pub fn new(web3: Web3, current_block_stream: CurrentBlockWatcher) -> Self {
        Self {
            storage: Default::default(),
            web3,
            current_block_stream,
        }
    }

    /// Starts indexing CoW AMMs deployed by the provided `factory` address.
    /// Interfacing with the CoW AMM happens via the
    /// [`contracts::CowAmmLegacyHelper`] deployed at `helper_contract`.
    pub async fn add_listener(
        &self,
        deployment_block: u64,
        factory: Address,
        helper_contract: Address,
    ) {
        let storage = Storage::new(
            deployment_block,
            CowAmmLegacyHelper::at(&self.web3, helper_contract),
        );
        self.storage.write().await.push(storage.clone());

        let indexer = Factory {
            web3: self.web3.clone(),
            address: factory,
        };
        let event_handler = EventHandler::new(Arc::new(self.web3.clone()), indexer, storage, None);
        let token_balance_maintainer =
            EmptyPoolRemoval::new(self.storage.clone(), self.web3.clone());
        let maintainers: Vec<Arc<dyn Maintaining>> = vec![
            Arc::new(Mutex::new(event_handler)),
            Arc::new(token_balance_maintainer),
        ];
        let service_maintainer = ServiceMaintenance::new(maintainers);
        tokio::task::spawn(
            service_maintainer.run_maintenance_on_new_block(self.current_block_stream.clone()),
        );
    }

    /// Returns all the deployed CoW AMMs
    pub async fn amms(&self) -> Vec<Arc<Amm>> {
        let mut result = vec![];
        let lock = self.storage.read().await;
        for cache in &*lock {
            result.extend(cache.cow_amms().await);
        }
        result
    }
}
