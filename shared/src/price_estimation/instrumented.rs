use super::{Estimate, PriceEstimating, PriceEstimationError, Query};
use std::sync::Arc;

/// An instrumented price estimator.
pub struct InstrumentedPriceEstimator<T> {
    inner: T,
    name: String,
    metrics: Arc<dyn Metrics>,
}

impl<T> InstrumentedPriceEstimator<T>
where
    T: PriceEstimating,
{
    /// Wraps an existing price estimator in an instrumented one.
    pub fn new(inner: T, name: impl Into<String>, metrics: Arc<dyn Metrics>) -> Self {
        Self {
            inner,
            name: name.into(),
            metrics,
        }
    }
}

#[async_trait::async_trait]
impl<T> PriceEstimating for InstrumentedPriceEstimator<T>
where
    T: PriceEstimating,
{
    async fn estimates(
        &self,
        queries: &[Query],
    ) -> Vec<anyhow::Result<Estimate, PriceEstimationError>> {
        let results = self.inner.estimates(queries).await;
        for result in &results {
            self.metrics.price_estimated(&self.name, result.is_ok());
        }
        results
    }
}

/// Metrics used by price estimators.
#[cfg_attr(test, mockall::automock)]
pub trait Metrics: Send + Sync + 'static {
    fn price_estimated(&self, name: &str, success: bool);
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

        let instrumented = InstrumentedPriceEstimator::new(estimator, "foo", Arc::new(metrics));
        let _ = instrumented.estimates(&queries).await;
    }
}
