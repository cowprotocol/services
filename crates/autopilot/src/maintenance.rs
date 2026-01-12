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
    tokio::sync::Mutex,
};

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
    /// On which block we last ran an update successfully.
    last_processed: Mutex<BlockInfo>,
    /// Limits the amount of time the autopilot may spend running the
    /// maintenance logic between 2 auctions. When this times out we prefer
    /// running a not fully updated auction over stalling the protocol any
    /// further.
    timeout: Duration,
}

impl Maintenance {
    pub fn new(
        settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
        db_cleanup: Postgres,
        timeout: Duration,
    ) -> Self {
        Self {
            settlement_indexer,
            db_cleanup,
            cow_amm_indexer: Default::default(),
            last_processed: Default::default(),
            timeout,
        }
    }

    /// Runs all update tasks in a coordinated manner to ensure the system
    /// has a consistent state.
    pub async fn update(&self, new_block: &BlockInfo) {
        let mut last_block = self.last_processed.lock().await;
        metrics().last_seen_block.set(new_block.number);
        if last_block.number > new_block.number || last_block.hash == new_block.hash {
            // `new_block` is neither newer than `last_block` nor a reorg
            return;
        }

        let start = Instant::now();

        if let Err(err) = tokio::time::timeout(self.timeout, self.update_inner()).await {
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
        let _timer =
            observe::metrics::metrics().on_auction_overhead_start("autopilot", "maintenance_total");
        tokio::try_join!(
            Self::timed_future(
                "settlement_indexer",
                self.settlement_indexer.run_maintenance()
            ),
            Self::timed_future("db_cleanup", self.db_cleanup.run_maintenance()),
        )?;

        Ok(())
    }

    /// Registers all maintenance tasks that are necessary to correctly support
    /// ethflow orders.
    pub fn spawn_ethflow_indexer(&mut self, ethflow_indexer: EthflowIndexer) {
        tokio::task::spawn(async move {
            loop {
                let _ =
                    Self::timed_future("ethflow_indexer", ethflow_indexer.run_maintenance()).await;
                tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
            }
        });
    }

    pub fn with_cow_amms(&mut self, registry: &cow_amm::Registry) {
        self.cow_amm_indexer = registry.maintenance_tasks().clone();
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

    /// Spawns a background task that runs on every new block but also
    /// at least after every `update_interval`.
    pub fn spawn_cow_amm_indexing_task(self_: Arc<Self>, current_block: CurrentBlockWatcher) {
        tokio::task::spawn(async move {
            let mut stream = into_stream(current_block);
            loop {
                let _ = match stream.next().await {
                    Some(block) => {
                        metrics().last_seen_block.set(block.number);
                        block
                    }
                    None => panic!("block stream terminated unexpectedly"),
                };

                // TODO: move this back into `Self::update_inner()` once we
                // store cow amms in the DB to avoid incredibly slow restarts.
                let _ = Self::timed_future(
                    "cow_amm_indexer",
                    futures::future::try_join_all(
                        self_
                            .cow_amm_indexer
                            .iter()
                            .map(|indexer| async move { indexer.run_maintenance().await }),
                    ),
                )
                .await;
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
