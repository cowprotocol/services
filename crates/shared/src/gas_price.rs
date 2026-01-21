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

        // do not use effective_gas_price here because it would duplicate the estimate call
        let base_fee = self.inner.base_fee().await?;
        self.metrics.base_fee.set(base_fee.unwrap_or(0) as i64);

        let effective_gas_price = calc_effective_gas_price(
            estimate.max_fee_per_gas,
            estimate.max_priority_fee_per_gas,
            base_fee,
        );

        tracing::info!(
            "estimate: {estimate:?}, base fee: {base_fee:?}, effective gas price: {effective_gas_price}"
        );

        self.metrics.gas_price.set(effective_gas_price as f64 / 1e9);
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
    /// Last measured base fee
    base_fee: prometheus::IntGauge,
}
