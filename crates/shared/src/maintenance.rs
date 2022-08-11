use crate::current_block::{self, Block, CurrentBlockStream};
use anyhow::{ensure, Result};
use futures::{future::join_all, Stream, StreamExt};
use std::sync::Arc;
use tracing::Instrument;

/// Collects all service components requiring maintenance on each new block
pub struct ServiceMaintenance {
    pub maintainers: Vec<Arc<dyn Maintaining>>,
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
    async fn run_maintenance_for_block_stream(self, block_stream: impl Stream<Item = Block>) {
        futures::pin_mut!(block_stream);

        let metrics = Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap();

        while let Some(block) = block_stream.next().await {
            tracing::debug!(
                "running maintenance on block number {:?} hash {:?}",
                block.number,
                block.hash
            );

            let block = block.number.unwrap_or_default().as_u64();
            metrics.last_seen_block.set(block as _);

            if self
                .run_maintenance()
                .instrument(tracing::debug_span!("maintenance", block))
                .await
                .is_ok()
            {
                metrics.last_updated_block.set(block as _);
            }
        }
    }

    pub async fn run_maintenance_on_new_block(self, current_block_stream: CurrentBlockStream) -> ! {
        self.run_maintenance_for_block_stream(current_block::into_stream(current_block_stream))
            .await;
        unreachable!()
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "maintenance")]
struct Metrics {
    /// Service maintenance last seen block.
    #[metric()]
    last_seen_block: prometheus::IntGauge,

    /// Service maintenance last seen block.
    #[metric()]
    last_updated_block: prometheus::IntGauge,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;

    #[tokio::test]
    async fn run_maintenance_ignores_errors() {
        let mut ok_mock_maintenance = MockMaintaining::new();
        let mut err_mock_maintenance = MockMaintaining::new();
        ok_mock_maintenance
            .expect_run_maintenance()
            .times(1)
            .returning(|| Ok(()));
        err_mock_maintenance
            .expect_run_maintenance()
            .times(1)
            .returning(|| bail!("Failed maintenance"));

        let service_maintenance = ServiceMaintenance {
            maintainers: vec![
                Arc::new(ok_mock_maintenance),
                Arc::new(err_mock_maintenance),
            ],
        };

        assert!(service_maintenance.run_maintenance().await.is_ok());
    }

    #[tokio::test]
    async fn block_stream_maintenance() {
        let block_count = 2;
        let mut mock_maintenance = MockMaintaining::new();
        // Mock interface is responsible for assertions here.
        // Will panic if run_maintenance is not called exactly `block_count` times.
        mock_maintenance
            .expect_run_maintenance()
            .times(block_count)
            .returning(|| Ok(()));
        let service_maintenance = ServiceMaintenance {
            maintainers: vec![Arc::new(mock_maintenance)],
        };

        let block_stream = futures::stream::repeat(Block::default()).take(block_count);
        service_maintenance
            .run_maintenance_for_block_stream(block_stream)
            .await;
    }
}
