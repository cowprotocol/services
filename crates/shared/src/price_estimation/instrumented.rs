use crate::price_estimation::{PriceEstimating, PriceEstimationError, Query};
use futures::stream::StreamExt;
use prometheus::{HistogramVec, IntCounterVec};
use std::time::Instant;

/// An instrumented price estimator.
pub struct InstrumentedPriceEstimator {
    inner: Box<dyn PriceEstimating>,
    name: String,
    metrics: &'static Metrics,
}

impl InstrumentedPriceEstimator {
    /// Wraps an existing price estimator in an instrumented one.
    pub fn new(inner: Box<dyn PriceEstimating>, name: String) -> Self {
        let metrics = Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap();
        for result in ["success", "failure"] {
            metrics
                .price_estimates
                .with_label_values(&[name.as_str(), result])
                .reset();
        }
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
                .price_estimation_times
                .with_label_values(&[self.name.as_str()])
                .observe(start.elapsed().as_secs_f64());
        };
        self.inner
            .estimates(queries)
            .inspect(move |result| {
                let success = !matches!(&result.1, Err(PriceEstimationError::Other(_)));
                let result = if success { "success" } else { "failure" };
                self.metrics
                    .price_estimates
                    .with_label_values(&[self.name.as_str(), result])
                    .inc();
            })
            .chain(futures::stream::once(measure_time).filter_map(|_| async { None }))
            .boxed()
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// price estimates
    #[metric(labels("estimator_type", "result"))]
    price_estimates: IntCounterVec,

    /// price estimation times
    #[metric(labels("estimator_type"))]
    price_estimation_times: HistogramVec,
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
    use model::order::OrderKind;

    #[tokio::test]
    async fn records_metrics_for_each_query() {
        let queries = [
            Query {
                from: None,
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                in_amount: 3.into(),
                kind: OrderKind::Sell,
            },
            Query {
                from: None,
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

        let instrumented = InstrumentedPriceEstimator::new(Box::new(estimator), "foo".to_string());
        let _ = vec_estimates(&instrumented, &queries).await;

        for result in &["success", "failure"] {
            let observed = instrumented
                .metrics
                .price_estimates
                .with_label_values(&["foo", result])
                .get();
            assert_eq!(observed, 1);
        }
        let observed = instrumented
            .metrics
            .price_estimation_times
            .with_label_values(&["foo"])
            .get_sample_count();
        assert_eq!(observed, 1);
    }
}
