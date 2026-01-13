//! Module defining an instrumented Ethereum gas price estimator.
//!
//! This allows us to keep track of historic gas prices in Grafana and do things
//! like alert when gas prices get too high as well as detect spikes and other
//! anomalies.

use {
    crate::gas_price_estimation::{GasPriceEstimating, price::GasPrice1559},
    anyhow::Result,
    tracing::instrument,
};

/// An instrumented gas price estimator that wraps an inner one.
pub struct InstrumentedGasEstimator<T> {
    inner: T,
    metrics: &'static Metrics,
}

impl<T> InstrumentedGasEstimator<T>
where
    T: GasPriceEstimating,
{
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
        }
    }
}

#[async_trait::async_trait]
impl<T> GasPriceEstimating for InstrumentedGasEstimator<T>
where
    T: GasPriceEstimating,
{
    #[instrument(skip_all)]
    async fn estimate(&self) -> Result<GasPrice1559> {
        let estimate = self.inner.estimate().await?;
        self.metrics
            .gas_price
            .set(estimate.effective_gas_price() / 1e9);
        Ok(estimate)
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Last measured gas price in gwei
    gas_price: prometheus::Gauge,
}
