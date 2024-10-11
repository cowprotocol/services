use {
    crate::{
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
    },
    anyhow::Result,
    ethrpc::block_stream::BlockInfo,
    prometheus::{
        core::{AtomicU64, GenericGauge},
        HistogramVec,
        IntCounterVec,
    },
    shared::maintenance::Maintaining,
    std::{future::Future, sync::Arc},
    tokio::sync::Mutex,
};

/// Coordinates all the updates that need to run a new block
/// to ensure a consistent view of the system.
pub struct Maintenance {
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
        settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
        db_cleanup: Postgres,
    ) -> Self {
        Self {
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
            Self::timed_future(
                "settlement_indexer",
                self.settlement_indexer.run_maintenance()
            ),
            Self::timed_future("db_cleanup", self.db_cleanup.run_maintenance()),
            Self::timed_future("ethflow_indexer", self.index_ethflow_orders()),
            Self::timed_future(
                "cow_amm_indexer",
                futures::future::try_join_all(
                    self.cow_amm_indexer
                        .iter()
                        .cloned()
                        .map(|indexer| async move { indexer.run_maintenance().await }),
                )
            ),
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

    /// Runs the future and collects runtime metrics.
    async fn timed_future<T>(label: &str, fut: impl Future<Output = T>) -> T {
        let _timer = metrics()
            .maintenance_stage_time
            .with_label_values(&[label])
            .start_timer();
        fut.await
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

    /// Autopilot maintenance stage time
    #[metric(
        labels("stage"),
        buckets(0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 1, 1.5, 2.0, 2.5, 3, 3.5, 4)
    )]
    maintenance_stage_time: HistogramVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}
