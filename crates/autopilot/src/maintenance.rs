use {
    crate::{
        boundary::events::settlement::{GPv2SettlementContract, Indexer},
        database::{
            Postgres,
            onchain_order_events::{
                OnchainOrderParser,
                ethflow_events::{EthFlowData, EthFlowDataForDb},
                event_retriever::CoWSwapOnchainOrdersContract,
            },
        },
        event_updater::EventUpdater,
    },
    anyhow::Result,
    ethrpc::block_stream::{BlockInfo, CurrentBlockWatcher, into_stream},
    futures::StreamExt,
    prometheus::{
        HistogramVec,
        IntCounterVec,
        core::{AtomicU64, GenericGauge},
    },
    shared::maintenance::Maintaining,
    std::{
        future::Future,
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::watch,
    tokio_stream::wrappers::WatchStream,
};

/// Component to sync with the maintenance logic that runs in a background task.
/// This allows us to run the maintenance logic ASAP but still wait for it to
/// finish in a convenient manner.
#[derive(Clone)]
pub struct MaintenanceSync {
    /// How long the autopilot wants to wait at most.
    timeout: Duration,
    last_processed_block: watch::Receiver<u64>,
}

impl MaintenanceSync {
    pub async fn wait_until_block_processed(&self, block: u64) {
        let _timer = observe::metrics::metrics()
            .on_auction_overhead_start("autopilot", "wait_for_maintenance");

        if let Err(_timeout) = tokio::time::timeout(self.timeout, self.wait_inner(block)).await {
            tracing::debug!("timed out waiting for maintenance");
        }
    }

    async fn wait_inner(&self, target_block: u64) {
        if *self.last_processed_block.borrow() >= target_block {
            return;
        }

        let mut stream = WatchStream::new(self.last_processed_block.clone());
        loop {
            let processed_block = stream.next().await.unwrap();
            if processed_block >= target_block {
                return;
            }
        }
    }
}

/// Coordinates all the updates that need to run a new block
/// to ensure a consistent view of the system.
pub struct Maintenance {
    /// Indexes and persists all events emited by the settlement contract.
    settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
    /// Used for periodic cleanup tasks to not have the DB overflow with old
    /// data.
    db_cleanup: Postgres,
    /// All indexing tasks to keep cow amms up to date.
    cow_amm_indexer: Vec<Arc<dyn Maintaining>>,
    /// Tasks to index ethflow orders that were submitted onchain.
    ethflow_indexer: Vec<EthflowIndexer>,
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
            ethflow_indexer: Default::default(),
        }
    }

    /// Spawns a background task continously processing the latest block.
    /// Returns a `[MaintenanceSync]` that handles waiting for a specific
    /// block to be processed.
    pub fn spawn_maintenance_task(
        self,
        blocks: CurrentBlockWatcher,
        timeout: Duration,
    ) -> MaintenanceSync {
        let (sender, receiver) = watch::channel(blocks.borrow().number);

        tokio::task::spawn(async move {
            let mut stream = into_stream(blocks);
            loop {
                let block = stream
                    .next()
                    .await
                    .expect("block stream terminated unexpectedly");
                self.index_until_block(block, &sender).await;
            }
        });

        MaintenanceSync {
            last_processed_block: receiver,
            timeout,
        }
    }

    async fn index_until_block(&self, block: BlockInfo, last_processed_block: &watch::Sender<u64>) {
        metrics().last_seen_block.set(block.number);
        let start = Instant::now();

        if let Err(err) = self.update_inner().await {
            tracing::warn!(?err, block = block.number, "failed to run maintenance");
            metrics().updates.with_label_values(&["error"]).inc();
            return;
        }

        tracing::info!(
            block = block.number,
            time = ?start.elapsed(),
            "successfully ran maintenance task"
        );
        metrics().last_updated_block.set(block.number);
        metrics().updates.with_label_values(&["success"]).inc();
        if let Err(err) = last_processed_block.send(block.number) {
            tracing::warn!(?err, "nobody listening for processed blocks anymore");
        }
    }

    async fn update_inner(&self) -> Result<()> {
        let _timer =
            observe::metrics::metrics().on_auction_overhead_start("autopilot", "maintenance_total");
        tokio::try_join!(
            Self::timed_future(
                "settlement_indexer",
                self.settlement_indexer.run_maintenance()
            ),
            Self::timed_future("db_cleanup", self.db_cleanup.run_maintenance()),
            Self::timed_future(
                "cow_amm_indexer",
                futures::future::try_join_all(
                    self.cow_amm_indexer
                        .iter()
                        .map(|indexer| indexer.run_maintenance()),
                ),
            ),
            Self::timed_future(
                "ethflow_indexer",
                futures::future::try_join_all(
                    self.ethflow_indexer
                        .iter()
                        .map(|indexer| indexer.run_maintenance()),
                ),
            ),
        )?;

        Ok(())
    }

    /// Registers all maintenance tasks that are necessary to correctly support
    /// ethflow orders.
    pub fn add_ethflow_indexer(&mut self, ethflow_indexer: EthflowIndexer) {
        self.ethflow_indexer.push(ethflow_indexer);
    }

    /// Registers all maintenance tasks that are necessary to correctly support
    /// CoW AMMs.
    pub fn add_cow_amm_indexer(&mut self, registry: &cow_amm::Registry) {
        self.cow_amm_indexer
            .extend(registry.maintenance_tasks().clone());
    }

    /// Runs the future and collects runtime metrics.
    async fn timed_future<T>(label: &str, fut: impl Future<Output = T>) -> T {
        let _timer = metrics()
            .maintenance_stage_time
            .with_label_values(&[label])
            .start_timer();
        let _timer2 = observe::metrics::metrics().on_auction_overhead_start("autopilot", label);
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
