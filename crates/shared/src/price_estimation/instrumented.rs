use {
    crate::price_estimation::{PriceEstimating, PriceEstimationError, Query},
    futures::future::FutureExt,
    prometheus::{HistogramVec, IntCounterVec},
    std::{sync::Arc, time::Instant},
    tracing::Instrument,
};

/// An instrumented price estimator.
pub struct InstrumentedPriceEstimator {
    inner: Box<dyn PriceEstimating>,
    name: String,
    metrics: &'static Metrics,
}

impl InstrumentedPriceEstimator {
    /// Wraps an existing price estimator in an instrumented one.
    pub fn new(inner: Box<dyn PriceEstimating>, name: String) -> Self {
        let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();
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

impl PriceEstimating for InstrumentedPriceEstimator {
    fn estimate(
        &self,
        query: Arc<Query>,
    ) -> futures::future::BoxFuture<'_, super::PriceEstimateResult> {
        async {
            tracing::info!("newlog InstrumentedPriceEstomator query={:?}", query);
            let start = Instant::now();
            let estimate = self.inner.estimate(query).await;
            self.metrics
                .price_estimation_times
                .with_label_values(&[self.name.as_str()])
                .observe(start.elapsed().as_secs_f64());

            let success = !matches!(&estimate, Err(PriceEstimationError::EstimatorInternal(_)));
            let result = if success { "success" } else { "failure" };
            self.metrics
                .price_estimates
                .with_label_values(&[self.name.as_str(), result])
                .inc();

            estimate
        }
        .instrument(tracing::info_span!("estimator", name = &self.name,))
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
    use {
        super::*,
        crate::price_estimation::{Estimate, MockPriceEstimating, PriceEstimationError},
        anyhow::anyhow,
        ethcontract::H160,
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
    };

    #[tokio::test]
    async fn records_metrics_for_each_query() {
        let queries = [
            Arc::new(Query {
                verification: None,
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                in_amount: NonZeroU256::try_from(3).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: None,
                sell_token: H160([4; 20]),
                buy_token: H160([5; 20]),
                in_amount: NonZeroU256::try_from(6).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
        ];

        let mut estimator = MockPriceEstimating::new();
        let expected_query = queries[0].clone();
        estimator
            .expect_estimate()
            .times(1)
            .withf(move |q| *q == expected_query)
            .returning(|_| async { Ok(Estimate::default()) }.boxed());
        let expected_query = queries[1].clone();
        estimator
            .expect_estimate()
            .times(1)
            .withf(move |q| *q == expected_query)
            .returning(|_| {
                async { Err(PriceEstimationError::EstimatorInternal(anyhow!(""))) }.boxed()
            });

        let instrumented = InstrumentedPriceEstimator::new(Box::new(estimator), "foo".to_string());

        let _ = instrumented.estimate(queries[0].clone()).await;
        let _ = instrumented.estimate(queries[1].clone()).await;

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
        assert_eq!(observed, 2);
    }
}
