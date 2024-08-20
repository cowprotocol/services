//! Coordinates all the updates that need to run a new block
//! to ensure a consistent view of the system.

use {
    crate::{
        boundary::events::settlement::{GPv2SettlementContract, Indexer},
        database::{
            ethflow_events::event_retriever::EthFlowRefundRetriever,
            onchain_order_events::{
                ethflow_events::{EthFlowData, EthFlowDataForDb},
                event_retriever::CoWSwapOnchainOrdersContract,
                OnchainOrderParser,
            },
            Postgres,
        },
        event_updater::EventUpdater,
        solvable_orders::SolvableOrdersCache,
    },
    ethrpc::block_stream::{into_stream, BlockInfo, CurrentBlockWatcher},
    futures::StreamExt,
    shared::maintenance::Maintaining,
    std::sync::{Arc, Mutex},
};

pub struct Maintenance {
    /// Set of orders that make up the current auction.
    orders_cache: Arc<SolvableOrdersCache>,
    /// Indexes and persists all events emited by the settlement contract.
    settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
    /// Indexes ethflow orders (orders selling native ETH).
    ethflow_indexer: Option<EthflowIndexer>,
    /// Indexes refunds issued for unsettled ethflow orders.
    refund_indexer: Option<EventUpdater<Postgres, EthFlowRefundRetriever>>,
    /// On which block we last ran an update successfully.
    last_processed: Mutex<BlockInfo>,
}

impl Maintenance {
    pub fn new(
        orders_cache: Arc<SolvableOrdersCache>,
        settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
    ) -> Self {
        Self {
            orders_cache,
            settlement_indexer,
            refund_indexer: None,
            ethflow_indexer: None,
            last_processed: Default::default(),
        }
    }

    /// Registers all maintenance tasks that are necessary to correctly support
    /// ethflow orders.
    pub fn with_ethflow(
        &mut self,
        ethflow_indexer: EthflowIndexer,
        refund_indexer: EventUpdater<Postgres, EthFlowRefundRetriever>,
    ) {
        self.ethflow_indexer = Some(ethflow_indexer);
        self.refund_indexer = Some(refund_indexer);
    }

    /// Runs all update tasks in a coordinated manner to ensure the system
    /// has a consistent state.
    pub async fn update(&self, new_block: &BlockInfo) {
        {
            let last_block = self.last_processed.lock().unwrap();
            if last_block.number > new_block.number || last_block.hash == new_block.hash {
                // `new_block` is neither newer than `last_block` nor a reorg
                return;
            }
        }

        // All these can run independently of each other.
        let _ = tokio::join!(
            self.settlement_indexer.run_maintenance(),
            self.index_refunds(),
            self.index_ethflow_orders(),
        );

        // Only update solvable orders after all other
        // events got processed.
        let _ = self.orders_cache.update(new_block.number).await;
        *self.last_processed.lock().unwrap() = *new_block;
    }

    async fn index_refunds(&self) {
        if let Some(indexer) = &self.refund_indexer {
            let _ = indexer.run_maintenance().await;
        }
    }

    async fn index_ethflow_orders(&self) {
        if let Some(indexer) = &self.ethflow_indexer {
            let _ = indexer.run_maintenance().await;
        }
    }

    /// Spawns a background task that runs updates when new blocks are seen.
    pub fn spawn_background_task(self_: Arc<Self>, current_block: CurrentBlockWatcher) {
        tokio::task::spawn(async move {
            let mut stream = into_stream(current_block);
            while let Some(block) = stream.next().await {
                self_.update(&block).await;
            }
            panic!("block stream terminated unexpectedly");
        });
    }
}

type EthflowIndexer =
    EventUpdater<OnchainOrderParser<EthFlowData, EthFlowDataForDb>, CoWSwapOnchainOrdersContract>;
