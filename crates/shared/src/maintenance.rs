use crate::current_block::{self, Block, CurrentBlockStream};
use anyhow::{ensure, Result};
use futures::{future::join_all, Stream, StreamExt as _};
use std::{sync::Arc, time::Duration};
use tokio::time;
use tracing::Instrument as _;

/// Collects all service components requiring maintenance on each new block
pub struct ServiceMaintenance {
    maintainers: Vec<Arc<dyn Maintaining>>,
    retry_delay: Duration,
}

impl ServiceMaintenance {
    pub fn new(maintainers: Vec<Arc<dyn Maintaining>>) -> Self {
        Self {
            maintainers,
            retry_delay: Duration::from_secs(1),
        }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait Maintaining: Send + Sync {
    async fn run_maintenance(&self) -> Result<()>;
}

#[async_trait::async_trait]
impl Maintaining for ServiceMaintenance {
    async fn run_maintenance(&self) -> Result<()> {
        let mut no_error = true;
        for result in join_all(self.maintainers.iter().map(|m| m.run_maintenance())).await {
            if let Err(err) = result {
                tracing::warn!("Service Maintenance Error: {:?}", err);
                no_error = false;
            }
        }

        ensure!(no_error, "maintenance encounted one or more errors");
        Ok(())
    }
}

impl ServiceMaintenance {
    pub async fn run_maintenance_for_blocks(self, blocks: impl Stream<Item = Block>) {
        let metrics = Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap();
        for label in ["success", "failure"] {
            metrics.runs.with_label_values(&[label]).reset();
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
            tracing::debug!(
                ?block.number, ?block.hash,
                "running maintenance",
            );

            let block_number = block.number.unwrap_or_default().as_u64();

            metrics.last_seen_block.set(block_number as _);

            if let Err(err) = self
                .run_maintenance()
                .instrument(tracing::debug_span!("maintenance", block = block_number))
                .await
            {
                tracing::debug!(
                    ?block.number, ?block.hash, ?err,
                    "maintenance failed; queuing retry",
                );

                metrics.runs.with_label_values(&["failure"]).inc();
                retry_block = Some(block);
                continue;
            }

            metrics.last_updated_block.set(block_number as _);
            metrics.runs.with_label_values(&["success"]).inc();
        }
    }

    pub async fn run_maintenance_on_new_block(self, current_block_stream: CurrentBlockStream) -> ! {
        self.run_maintenance_for_blocks(current_block::into_stream(current_block_stream))
            .await;
        panic!("block stream unexpectedly dropped");
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "maintenance")]
struct Metrics {
    /// Service maintenance last seen block.
    #[metric()]
    last_seen_block: prometheus::IntGauge,

    /// Service maintenance last successfully updated block.
    #[metric()]
    last_updated_block: prometheus::IntGauge,

    /// Service maintenance error counter
    #[metric(labels("result"))]
    runs: prometheus::IntCounterVec,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use futures::stream;
    use mockall::Sequence;

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
        ok2_mock_maintenance
            .expect_run_maintenance()
            .times(1)
            .returning(|| Ok(()));

        let service_maintenance = ServiceMaintenance {
            maintainers: vec![
                Arc::new(ok1_mock_maintenance),
                Arc::new(err_mock_maintenance),
                Arc::new(ok2_mock_maintenance),
            ],
            retry_delay: Duration::default(),
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
            .expect_run_maintenance()
            .times(block_count)
            .returning(|| Ok(()));

        let service_maintenance = ServiceMaintenance {
            maintainers: vec![Arc::new(mock_maintenance)],
            retry_delay: Duration::default(),
        };

        let block_stream = stream::repeat(Block::default()).take(block_count);
        service_maintenance
            .run_maintenance_for_blocks(block_stream)
            .await;
    }

    #[tokio::test]
    async fn block_stream_retries_failed_blocks() {
        crate::tracing::initialize("debug", tracing::Level::ERROR.into());

        let mut mock_maintenance = MockMaintaining::new();
        let mut sequence = Sequence::new();
        mock_maintenance
            .expect_run_maintenance()
            .return_once(|| bail!("test"))
            .times(1)
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

        let service_maintenance = ServiceMaintenance {
            maintainers: vec![Arc::new(mock_maintenance)],
            retry_delay: Duration::default(),
        };

        let block_stream = async_stream::stream! {
            yield Block::default();

            // Wait a bit to trigger a retry and not just go to the next block
            time::sleep(Duration::from_millis(10)).await;
            yield Block::default();
        };
        service_maintenance
            .run_maintenance_for_blocks(block_stream)
            .await;
    }
}
