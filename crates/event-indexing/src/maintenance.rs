use {
    anyhow::{Result, ensure},
    ethrpc::block_stream::{self, BlockInfo, CurrentBlockWatcher},
    futures::{Stream, StreamExt as _, future::join_all},
    std::{sync::Weak, time::Duration},
    tokio::time,
    tracing::Instrument as _,
};

/// Collects all service components requiring maintenance on each new block.
pub struct ServiceMaintenance {
    maintainers: Vec<Weak<dyn Maintaining>>,
    retry_delay: Duration,
    metrics: &'static Metrics,
}

impl ServiceMaintenance {
    pub fn new(maintainers: Vec<Weak<dyn Maintaining>>) -> Self {
        Self {
            maintainers,
            retry_delay: Duration::from_secs(1),
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
        }
    }

    async fn run_maintenance_for_blocks(mut self, blocks: impl Stream<Item = BlockInfo>) -> Result<()> {
        for maintainer in self.maintainers.iter().filter_map(Weak::upgrade) {
            self.metrics
                .runs
                .with_label_values(&["failure", maintainer.name()])
                .reset();
        }

        let blocks = blocks.fuse();
        futures::pin_mut!(blocks);

        let mut retry_block = None;

        while let Some(block) = match retry_block.take() {
            // We have a pending retry to process. First see if there is a new
            // block that becomes available within a certain retry delay, and if
            // there is, prefer that over the old outdated block.
            Some(block) => time::timeout(self.retry_delay, blocks.next())
                .await
                .unwrap_or(Some(block)),
            None => blocks.next().await,
        } {
            self.maintainers.retain(|m| m.strong_count() > 0);
            if !self.maintainers.is_empty() {
                tracing::debug!("no component needs maintenance anymore, terminating loop");
                return Ok(());
            }

            tracing::debug!(
                ?block.number, ?block.hash,
                "running maintenance",
            );

            self.metrics
                .last_seen_block
                .set(i64::try_from(block.number).unwrap_or(i64::MAX));

            if let Err(err) = self
                .run_maintenance()
                .instrument(tracing::debug_span!("maintenance", block = block.number))
                .await
            {
                tracing::debug!(
                    ?block.number, ?block.hash, ?err,
                    "maintenance failed; queuing retry",
                );

                retry_block = Some(block);
                continue;
            }

            self.metrics
                .last_updated_block
                .set(i64::try_from(block.number).unwrap_or(i64::MAX));
        }

        Err(anyhow::anyhow!("block stream terminated unexpectedly"))
    }

    pub async fn run_maintenance_on_new_block(self, current_block_stream: CurrentBlockWatcher) {
        self.run_maintenance_for_blocks(block_stream::into_stream(current_block_stream))
            .instrument(tracing::info_span!("service_maintenance"))
            .await
            .expect("maintenance task terminated with error");
    }

    async fn run_maintenance(&self) -> Result<()> {
        let mut no_error = true;
        for (result, maintainer) in join_all(
            self.maintainers
                .iter()
                .filter_map(Weak::upgrade)
                .map(|m| async move { (m.run_maintenance().await, m) }),
        )
        .await
        .into_iter()
        {
            if let Err(err) = result {
                let maintainer = maintainer.name();
                tracing::warn!(
                    "Service Maintenance Error for maintainer {}: {:?}",
                    maintainer,
                    err
                );
                self.metrics
                    .runs
                    .with_label_values(&["failure", maintainer])
                    .inc();

                no_error = false;
            }
        }

        ensure!(no_error, "maintenance encounted one or more errors");
        Ok(())
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait Maintaining: Send + Sync {
    async fn run_maintenance(&self) -> Result<()>;
    fn name(&self) -> &str;
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "maintenance")]
struct Metrics {
    /// Service maintenance last seen block.
    last_seen_block: prometheus::IntGauge,

    /// Service maintenance last successfully updated block.
    last_updated_block: prometheus::IntGauge,

    /// Service maintenance error counter
    #[metric(labels("result", "maintainer"))]
    runs: prometheus::IntCounterVec,
}

#[cfg(test)]
mod tests {
    use {super::*, anyhow::bail, futures::stream, mockall::Sequence, std::sync::Arc};

    #[tokio::test]
    async fn run_maintenance_no_early_exit_on_error() {
        let mut ok1_mock_maintenance = MockMaintaining::new();
        let mut err_mock_maintenance = MockMaintaining::new();
        let mut ok2_mock_maintenance = MockMaintaining::new();
        ok1_mock_maintenance
            .expect_run_maintenance()
            .times(1)
            .returning(|| Ok(()));
        err_mock_maintenance
            .expect_run_maintenance()
            .times(1)
            .returning(|| bail!("Failed maintenance"));
        err_mock_maintenance
            .expect_name()
            .times(1)
            .return_const("test".to_string());
        ok2_mock_maintenance
            .expect_run_maintenance()
            .times(1)
            .returning(|| Ok(()));

        let m1 = Arc::new(ok1_mock_maintenance) as Arc<dyn Maintaining>;
        let m2 = Arc::new(err_mock_maintenance) as Arc<dyn Maintaining>;
        let m3 = Arc::new(ok2_mock_maintenance) as Arc<dyn Maintaining>;

        let service_maintenance = ServiceMaintenance {
            maintainers: vec![
                Arc::downgrade(&m1),
                Arc::downgrade(&m2),
                Arc::downgrade(&m3),
            ],
            retry_delay: Duration::default(),
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
        };

        assert!(service_maintenance.run_maintenance().await.is_err());
    }

    #[tokio::test]
    async fn block_stream_maintenance() {
        let block_count = 5;

        // Mock interface is responsible for assertions here.
        // Will panic if run_maintenance is not called exactly `block_count` times.
        let mut mock_maintenance = MockMaintaining::new();
        mock_maintenance
            .expect_name()
            .times(1)
            .return_const("test".to_string());
        mock_maintenance
            .expect_run_maintenance()
            .times(block_count)
            .returning(|| Ok(()));

        let m1 = Arc::new(mock_maintenance) as Arc<dyn Maintaining>;
        let service_maintenance = ServiceMaintenance {
            maintainers: vec![Arc::downgrade(&m1)],
            retry_delay: Duration::default(),
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
        };

        let block_stream = stream::repeat(BlockInfo::default()).take(block_count);
        service_maintenance
            .run_maintenance_for_blocks(block_stream)
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn block_stream_retries_failed_blocks() {
        let obs_config = observe::Config::default().with_env_filter("debug");
        observe::tracing::init::initialize(&obs_config);

        let mut mock_maintenance = MockMaintaining::new();
        let mut sequence = Sequence::new();
        mock_maintenance
            .expect_name()
            .times(1)
            .return_const("test".to_string())
            .in_sequence(&mut sequence);
        mock_maintenance
            .expect_run_maintenance()
            .return_once(|| bail!("test"))
            .times(1)
            .in_sequence(&mut sequence);
        mock_maintenance
            .expect_name()
            .times(1)
            .return_const("test".to_string())
            .in_sequence(&mut sequence);
        mock_maintenance
            .expect_run_maintenance()
            .return_once(|| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);
        mock_maintenance
            .expect_run_maintenance()
            .return_once(|| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        let m1 = Arc::new(mock_maintenance) as Arc<dyn Maintaining>;
        let service_maintenance = ServiceMaintenance {
            maintainers: vec![Arc::downgrade(&m1)],
            retry_delay: Duration::default(),
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
        };

        let block_stream = async_stream::stream! {
            yield BlockInfo::default();

            // Wait a bit to trigger a retry and not just go to the next block
            time::sleep(Duration::from_millis(10)).await;
            yield BlockInfo::default();
        };
        service_maintenance
            .run_maintenance_for_blocks(block_stream)
            .await
            .unwrap_err();
    }

    /// Tests that the maintenance task terminates gracefully (`Ok(())`) when
    /// all managed sub-tasks indicate they no longer need to run.
    #[tokio::test]
    async fn task_terminates_gracefully() {
        let obs_config = observe::Config::default().with_env_filter("debug");
        observe::tracing::init::initialize(&obs_config);

        let mut m1 = MockMaintaining::new();
        m1.expect_name().return_const("test".to_string());
        m1.expect_run_maintenance().times(3).returning(|| Ok(()));

        let mut m2 = MockMaintaining::new();
        m2.expect_name().return_const("test".to_string());
        m2.expect_run_maintenance().times(7).returning(|| Ok(()));

        let m1 = Arc::new(m1) as Arc<dyn Maintaining>;
        let m2 = Arc::new(m2) as Arc<dyn Maintaining>;
        let service_maintenance = ServiceMaintenance {
            maintainers: vec![Arc::downgrade(&m1), Arc::downgrade(&m2)],
            retry_delay: Duration::default(),
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
        };

        let block_stream = async_stream::stream! {
            let mut i = 0;
            let mut m1 = Some(m1);
            let mut m2 = Some(m2);
            loop {
                if i == 3 {
                    // first drop m1 to verify that m2 keeps getting maintained while
                    // m1 no longer runs
                    m1.take();
                }
                if i == 7 {
                    // after m2 also gets dropped the whole service_maintenance terminates
                    // gracefully
                    m2.take();
                }

                yield BlockInfo::default();

                time::sleep(Duration::from_millis(10)).await;
                i += 1;
            }
        };
        service_maintenance
            .run_maintenance_for_blocks(block_stream)
            .await
            .expect("task terminated with error despite all maintainers getting dropped");
    }
}
