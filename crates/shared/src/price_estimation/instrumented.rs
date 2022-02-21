use super::{Estimate, PriceEstimating, PriceEstimationError, Query};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// An instrumented price estimator.
pub struct InstrumentedPriceEstimator {
    inner: Box<dyn PriceEstimating>,
    name: String,
    metrics: Arc<dyn Metrics>,
}

impl InstrumentedPriceEstimator {
    /// Wraps an existing price estimator in an instrumented one.
    pub fn new(inner: Box<dyn PriceEstimating>, name: String, metrics: Arc<dyn Metrics>) -> Self {
        metrics.initialize_estimator(&name);
        Self {
            inner,
            name,
            metrics,
        }
    }
}

#[async_trait::async_trait]
impl PriceEstimating for InstrumentedPriceEstimator {
    async fn estimates(
        &self,
        queries: &[Query],
    ) -> Vec<anyhow::Result<Estimate, PriceEstimationError>> {
        let start = Instant::now();
        let results = self.inner.estimates(queries).await;
        for result in &results {
            let success = !matches!(result, Err(PriceEstimationError::Other(_)));
            self.metrics.price_estimated(&self.name, success);
        }
        self.metrics
            .price_estimation_timed(&self.name, start.elapsed());
        results
    }
}

/// Metrics used by price estimators.
#[cfg_attr(test, mockall::automock)]
pub trait Metrics: Send + Sync + 'static {
    fn initialize_estimator(&self, name: &str);
    fn price_estimated(&self, name: &str, success: bool);
    fn price_estimation_timed(&self, name: &str, time: Duration);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::MockPriceEstimating;
    use anyhow::anyhow;
    use ethcontract::H160;
    use mockall::{predicate::*, Sequence};
    use model::order::OrderKind;

    #[tokio::test]
    async fn records_metrics_for_each_query() {
        let queries = [
            Query {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                in_amount: 3.into(),
                kind: OrderKind::Sell,
            },
            Query {
                sell_token: H160([4; 20]),
                buy_token: H160([5; 20]),
                in_amount: 6.into(),
                kind: OrderKind::Buy,
            },
        ];

        let mut estimator = MockPriceEstimating::new();
        estimator
            .expect_estimates()
            .times(1)
            .withf(move |q| q == queries)
            .returning(|_| {
                vec![
                    Ok(Estimate::default()),
                    Err(PriceEstimationError::Other(anyhow!(""))),
                ]
            });

        let mut metrics = MockMetrics::new();
        metrics
            .expect_initialize_estimator()
            .times(1)
            .with(eq("foo"))
            .return_const(());
        let mut seq = Sequence::new();
        metrics
            .expect_price_estimated()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq("foo"), eq(true))
            .return_const(());
        metrics
            .expect_price_estimated()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq("foo"), eq(false))
            .return_const(());
        metrics
            .expect_price_estimation_timed()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq("foo"), always())
            .return_const(());

        let instrumented = InstrumentedPriceEstimator::new(
            Box::new(estimator),
            "foo".to_string(),
            Arc::new(metrics),
        );
        let _ = instrumented.estimates(&queries).await;
    }
}
