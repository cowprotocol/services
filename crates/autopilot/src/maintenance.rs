use {
    crate::{
        arguments::RunLoopMode,
        boundary::events::settlement::{GPv2SettlementContract, Indexer},
        database::{
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
    anyhow::Result,
    ethrpc::block_stream::{into_stream, BlockInfo, CurrentBlockWatcher},
    futures::StreamExt,
    prometheus::{
        core::{AtomicU64, GenericGauge},
        IntCounterVec,
    },
    shared::maintenance::Maintaining,
    std::{sync::Arc, time::Duration},
    tokio::{sync::Mutex, time::timeout},
};

/// Coordinates all the updates that need to run a new block
/// to ensure a consistent view of the system.
pub struct Maintenance {
    /// Set of orders that make up the current auction.
    orders_cache: Arc<SolvableOrdersCache>,
    /// Indexes and persists all events emited by the settlement contract.
    settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
    /// Indexes ethflow orders (orders selling native ETH).
    ethflow_indexer: Option<EthflowIndexer>,
    /// Used for periodic cleanup tasks to not have the DB overflow with old
    /// data.
    db_cleanup: Postgres,
    /// All indexing tasks to keep cow amms up to date.
    cow_amm_indexer: Vec<Arc<dyn Maintaining>>,
    /// On which block we last ran an update successfully.
    last_processed: Mutex<BlockInfo>,
}

impl Maintenance {
    pub fn new(
        orders_cache: Arc<SolvableOrdersCache>,
        settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
        db_cleanup: Postgres,
    ) -> Self {
        Self {
            orders_cache,
            settlement_indexer,
            db_cleanup,
            cow_amm_indexer: Default::default(),
            ethflow_indexer: None,
            last_processed: Default::default(),
        }
    }

    /// Runs all update tasks in a coordinated manner to ensure the system
    /// has a consistent state.
    pub async fn update(&self, new_block: &BlockInfo) {
        let mut last_block = self.last_processed.lock().await;
        if last_block.number > new_block.number || last_block.hash == new_block.hash {
            // `new_block` is neither newer than `last_block` nor a reorg
            return;
        }

        let start = std::time::Instant::now();
        if let Err(err) = self.update_inner().await {
            tracing::warn!(?err, block = new_block.number, "failed to run maintenance");
            metrics().updates.with_label_values(&["error"]).inc();
            return;
        }
        tracing::info!(
            block = new_block.number,
            time = ?start.elapsed(),
            "successfully ran maintenance task"
        );

        metrics().updates.with_label_values(&["success"]).inc();
        metrics().last_updated_block.set(new_block.number);
        *last_block = *new_block;
    }

    async fn update_inner(&self) -> Result<()> {
        // All these can run independently of each other.
        tokio::try_join!(
            self.settlement_indexer.run_maintenance(),
            self.db_cleanup.run_maintenance(),
            self.index_ethflow_orders(),
            futures::future::try_join_all(
                self.cow_amm_indexer
                    .iter()
                    .cloned()
                    .map(|indexer| async move { indexer.run_maintenance().await })
            )
        )?;

        Ok(())
    }

    /// Registers all maintenance tasks that are necessary to correctly support
    /// ethflow orders.
    pub fn with_ethflow(&mut self, ethflow_indexer: EthflowIndexer) {
        self.ethflow_indexer = Some(ethflow_indexer);
    }

    pub fn with_cow_amms(&mut self, registry: &cow_amm::Registry) {
        self.cow_amm_indexer = registry.maintenance_tasks().clone();
    }

    async fn index_ethflow_orders(&self) -> Result<()> {
        if let Some(indexer) = &self.ethflow_indexer {
            return indexer.run_maintenance().await;
        }
        Ok(())
    }

    /// Spawns a background task that runs on every new block but also
    /// at least after every `update_interval`.
    pub fn spawn_background_task(
        self_: Arc<Self>,
        run_loop_mode: RunLoopMode,
        current_block: CurrentBlockWatcher,
        update_interval: Duration,
    ) {
        tokio::task::spawn(async move {
            match run_loop_mode {
                RunLoopMode::SyncToBlockchain => {
                    // Update last seen block metric only since everything else will be updated
                    // inside the runloop.
                    let mut stream = into_stream(current_block);
                    loop {
                        let next_update = timeout(update_interval, stream.next());
                        match next_update.await {
                            Ok(Some(block)) => {
                                metrics().last_seen_block.set(block.number);
                            }
                            Ok(None) => break,
                            Err(_timeout) => {}
                        };
                    }
                }
                RunLoopMode::Unsynchronized => {
                    let mut latest_block = *current_block.borrow();
                    let mut stream = into_stream(current_block);
                    loop {
                        let next_update = timeout(update_interval, stream.next());
                        let current_block = match next_update.await {
                            Ok(Some(block)) => {
                                metrics().last_seen_block.set(block.number);
                                block
                            }
                            Ok(None) => break,
                            Err(_timeout) => latest_block,
                        };
                        if let Err(err) = self_.update_inner().await {
                            tracing::warn!(?err, "failed to run background task successfully");
                        }
                        if let Err(err) = self_.orders_cache.update(current_block.number).await {
                            tracing::warn!(?err, "failed to update auction successfully");
                        }
                        latest_block = current_block;
                    }
                    panic!("block stream terminated unexpectedly");
                }
            }
        });
    }
}

type EthflowIndexer =
    EventUpdater<OnchainOrderParser<EthFlowData, EthFlowDataForDb>, CoWSwapOnchainOrdersContract>;

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "autopilot_maintenance")]
struct Metrics {
    /// Autopilot maintenance last seen block.
    last_seen_block: GenericGauge<AtomicU64>,

    /// Autopilot maintenance last successfully updated block.
    last_updated_block: GenericGauge<AtomicU64>,

    /// Autopilot maintenance error counter
    #[metric(labels("result"))]
    updates: IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}
