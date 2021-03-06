use crate::price_estimation::{PriceEstimating, PriceEstimationError, Query};
use futures::stream::StreamExt;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

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
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, super::PriceEstimateResult)> {
        let start = Instant::now();
        let measure_time = async move {
            self.metrics
                .price_estimation_timed(&self.name, start.elapsed());
        };
        self.inner
            .estimates(queries)
            .inspect(move |result| {
                let success = !matches!(&result.1, Err(PriceEstimationError::Other(_)));
                self.metrics.price_estimated(&self.name, success);
            })
            .chain(futures::stream::once(measure_time).filter_map(|_| async { None }))
            .boxed()
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
    use crate::price_estimation::{
        vec_estimates, Estimate, MockPriceEstimating, PriceEstimationError,
    };
    use anyhow::anyhow;
    use ethcontract::H160;
    use futures::StreamExt;
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
                futures::stream::iter([
                    Ok(Estimate::default()),
                    Err(PriceEstimationError::Other(anyhow!(""))),
                ])
                .enumerate()
                .boxed()
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
        let _ = vec_estimates(&instrumented, &queries).await;
    }
}
