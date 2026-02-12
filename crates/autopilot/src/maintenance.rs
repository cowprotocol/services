use {
    crate::{
        boundary::events::settlement::{GPv2SettlementContract, Indexer},
        database::{
            Postgres,
            ethflow_events::event_retriever::EthFlowRefundRetriever,
            onchain_order_events::{
                OnchainOrderParser,
                ethflow_events::{EthFlowData, EthFlowDataForDb},
                event_retriever::CoWSwapOnchainOrdersContract,
            },
        },
        domain::settlement,
        event_updater::EventUpdater,
    },
    anyhow::Result,
    ethrpc::block_stream::{BlockInfo, CurrentBlockWatcher, into_stream},
    futures::{FutureExt, StreamExt},
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
    tracing::Instrument,
};

/// Component to sync with the maintenance logic that runs in a background task.
/// This allows us to run the maintenance logic as soon as we see a new block
/// while still making the autopilot run loop only wait for updates that are
/// essential for building new auctions.
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
    ethflow_order_indexer: Vec<EthflowOrderIndexer>,
    /// Tasks to index ethflow refunds.
    ethflow_refund_indexer: Vec<EthflowRefundIndexer>,
    /// Component to correctly attribute a settlement to a proposed solution.
    settlement_observer: settlement::Observer,
}

impl Maintenance {
    pub fn new(
        settlement_indexer: EventUpdater<Indexer, GPv2SettlementContract>,
        db_cleanup: Postgres,
        settlement_observer: settlement::Observer,
    ) -> Self {
        Self {
            settlement_indexer,
            db_cleanup,
            cow_amm_indexer: Default::default(),
            ethflow_order_indexer: Default::default(),
            ethflow_refund_indexer: Default::default(),
            settlement_observer,
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
                self.index_until_block(block, &sender)
                    .instrument(tracing::info_span!(
                        "autopilot_maintenance",
                        block = block.number
                    ))
                    .await;
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

        if let Err(err) = self.run_essential_maintenance().await {
            tracing::warn!(?err, "failed to run essential maintenance");
            metrics().updates.with_label_values(&["error"]).inc();
            return;
        }

        tracing::info!(
            time = ?start.elapsed(),
            "successfully ran essential maintenance tasks"
        );
        metrics().last_updated_block.set(block.number);
        metrics().updates.with_label_values(&["success"]).inc();
        if let Err(err) = last_processed_block.send(block.number) {
            tracing::warn!(?err, "nobody listening for processed blocks anymore");
        }

        // only after we informed the run_loop that the essential updates are done we
        // kick off the optional maintenance tasks
        let start = Instant::now();
        if let Err(err) = self.run_optional_maintenance().await {
            tracing::warn!(?err, "failed to run optional maintenance");
            return;
        }
        tracing::info!(
            time = ?start.elapsed(),
            "successfully ran optional maintenance tasks"
        );
    }

    /// Runs all the maintenance tasks that are needed to ensure the next
    /// auction gets built using the most up-to-date information.
    async fn run_essential_maintenance(&self) -> Result<()> {
        let _timer = observe::metrics::metrics()
            .on_auction_overhead_start("autopilot", "maintenance_essential");
        tokio::try_join!(
            Self::timed_future(
                "settlement_indexer",
                self.settlement_indexer.run_maintenance()
            ),
            Self::timed_future(
                "cow_amm_indexer",
                futures::future::try_join_all(
                    self.cow_amm_indexer
                        .iter()
                        .map(|indexer| indexer.run_maintenance()),
                ),
            ),
            Self::timed_future(
                "ethflow_order_indexer",
                futures::future::try_join_all(
                    self.ethflow_order_indexer
                        .iter()
                        .map(|indexer| indexer.run_maintenance()),
                ),
            ),
        )?;

        Ok(())
    }

    /// Runs all the maintenance tasks that should run eventually but are not
    /// very time sensitive.
    async fn run_optional_maintenance(&self) -> Result<()> {
        let _timer = observe::metrics::metrics()
            .on_auction_overhead_start("autopilot", "maintenance_optional");
        tokio::try_join!(
            Self::timed_future("db_cleanup", self.db_cleanup.run_maintenance()),
            Self::timed_future(
                "ethflow_refund_indexer",
                futures::future::try_join_all(
                    self.ethflow_refund_indexer
                        .iter()
                        .map(|indexer| indexer.run_maintenance()),
                ),
            ),
            Self::timed_future(
                "settlement_attribution",
                self.settlement_observer
                    .post_process_outstanding_settlement_transactions()
                    .map(|_| Ok(()))
            )
        )?;

        Ok(())
    }

    /// Registers all maintenance tasks that are necessary to correctly support
    /// ethflow orders.
    pub fn add_ethflow_indexing(
        &mut self,
        order_indexer: EthflowOrderIndexer,
        refund_indexer: EthflowRefundIndexer,
    ) {
        self.ethflow_order_indexer.push(order_indexer);
        self.ethflow_refund_indexer.push(refund_indexer);
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

type EthflowOrderIndexer =
    EventUpdater<OnchainOrderParser<EthFlowData, EthFlowDataForDb>, CoWSwapOnchainOrdersContract>;

type EthflowRefundIndexer = EventUpdater<Postgres, EthFlowRefundRetriever>;

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
