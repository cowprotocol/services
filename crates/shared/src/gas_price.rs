//! Module defining an instrumented Ethereum gas price estimator.
//!
//! This allows us to keep track of historic gas prices in Grafana and do things
//! like alert when gas prices get too high as well as detect spikes and other
//! anomalies.

use {
    crate::gas_price_estimation::GasPriceEstimating,
    alloy::eips::eip1559::{Eip1559Estimation, calc_effective_gas_price},
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
    async fn estimate(&self) -> Result<Eip1559Estimation> {
        let estimate = self.inner.estimate().await?;
        self.metrics.gas_price.set(
            (calc_effective_gas_price(
                estimate.max_fee_per_gas,
                estimate.max_priority_fee_per_gas,
                None,
            ) / 10u128.pow(9)) as f64,
        );
        Ok(estimate)
    }

    async fn base_fee(&self) -> Result<Option<u64>> {
        self.inner.base_fee().await
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Last measured gas price in gwei
    gas_price: prometheus::Gauge,
}
