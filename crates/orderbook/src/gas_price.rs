//! Module defining an instrumented Ethereum gas price estimator.
//!
//! This allows us to keep track of historic gas prices in Grafana and do things
//! like alert when gas prices get too high as well as detect spikes and other
//! anomalies.

use anyhow::Result;
use gas_estimation::{GasPrice1559, GasPriceEstimating};
use std::{sync::Arc, time::Duration};

/// An instrumented gas price estimator that wraps an inner one.
pub struct InstrumentedGasEstimator<T> {
    inner: T,
    metrics: Arc<dyn Metrics>,
}

impl<T> InstrumentedGasEstimator<T>
where
    T: GasPriceEstimating,
{
    pub fn new(inner: T, metrics: Arc<dyn Metrics>) -> Self {
        Self { inner, metrics }
    }
}

#[async_trait::async_trait]
impl<T> GasPriceEstimating for InstrumentedGasEstimator<T>
where
    T: GasPriceEstimating,
{
    async fn estimate_with_limits(
        &self,
        gas_limit: f64,
        time_limit: Duration,
    ) -> Result<GasPrice1559> {
        // Instrumenting gas estimates with limits is hard. Since we don't use
        // it in the orderbook, lets leave this out for now.
        self.inner.estimate_with_limits(gas_limit, time_limit).await
    }

    async fn estimate(&self) -> Result<GasPrice1559> {
        let estimate = self.inner.estimate().await?;
        self.metrics.gas_price(estimate);
        Ok(estimate)
    }
}

/// Gas estimator metrics.
pub trait Metrics: Send + Sync + 'static {
    fn gas_price(&self, estimate: GasPrice1559);
}
