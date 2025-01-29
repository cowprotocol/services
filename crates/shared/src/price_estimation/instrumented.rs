use {
    crate::price_estimation::{
        native::{NativePriceEstimateResult, NativePriceEstimating},
        PriceEstimating,
        PriceEstimationError,
        Query,
    },
    ethcontract::jsonrpc::futures_util::future::BoxFuture,
    futures::future::FutureExt,
    primitive_types::H160,
    prometheus::{HistogramVec, IntCounterVec},
    std::{sync::Arc, time::Instant},
    tracing::Instrument,
};

/// An instrumented price estimator.
pub struct InstrumentedPriceEstimator<T> {
    inner: T,
    name: String,
    metrics: &'static Metrics,
}

impl<T> InstrumentedPriceEstimator<T> {
    /// Wraps an existing price estimator in an instrumented one.
    pub fn new(inner: T, name: String) -> Self {
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

    /// Determines the result of a price estimate, returning either "success" or
    /// the error reason
    fn estimate_result<B>(&self, estimate: Result<&B, &PriceEstimationError>) -> &str {
        // Count as a successful request if the answer is ok (no error) or if the error
        // is No Liquidity
        match estimate {
            Ok(_) => "success",
            Err(PriceEstimationError::NoLiquidity) => "success",
            Err(PriceEstimationError::UnsupportedToken { .. }) => "unsupported_token",
            Err(PriceEstimationError::UnsupportedOrderType(_)) => "unsupported_order_type",
            Err(PriceEstimationError::RateLimited) => "rate_limited",
            Err(PriceEstimationError::EstimatorInternal(_)) => "estimator_internal_error",
            Err(PriceEstimationError::ProtocolInternal(_)) => "protocol_internal_error",
        }
    }
}

impl<T: PriceEstimating> PriceEstimating for InstrumentedPriceEstimator<T> {
    fn estimate(
        &self,
        query: Arc<Query>,
    ) -> futures::future::BoxFuture<'_, super::PriceEstimateResult> {
        async {
            let start = Instant::now();
            let estimate = self.inner.estimate(query).await;
            self.metrics
                .price_estimation_times
                .with_label_values(&[self.name.as_str()])
                .observe(start.elapsed().as_secs_f64());
            let result = self.estimate_result(estimate.as_ref());
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

impl<T: NativePriceEstimating> NativePriceEstimating for InstrumentedPriceEstimator<T> {
    fn estimate_native_price(&self, token: H160) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let start = Instant::now();
            let estimate = self.inner.estimate_native_price(token).await;
            self.metrics
                .price_estimation_times
                .with_label_values(&[self.name.as_str()])
                .observe(start.elapsed().as_secs_f64());
            let result = self.estimate_result(estimate.as_ref());
            self.metrics
                .price_estimates
                .with_label_values(&[self.name.as_str(), result])
                .inc();

            estimate
        }
        .instrument(tracing::info_span!("native estimator", name = &self.name,))
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
                verification: Default::default(),
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                in_amount: NonZeroU256::try_from(3).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            }),
            Arc::new(Query {
                verification: Default::default(),
                sell_token: H160([4; 20]),
                buy_token: H160([5; 20]),
                in_amount: NonZeroU256::try_from(6).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }),
        ];

        let mut estimator = MockPriceEstimating::new();
        let expectations = vec![
            (0, Ok(Estimate::default())),
            (1, Err(PriceEstimationError::NoLiquidity)),
            (0, Err(PriceEstimationError::UnsupportedToken {
                token: H160([0; 20]),
                reason: "".to_string(),
            })),
            (1, Err(PriceEstimationError::UnsupportedOrderType("".to_string()))),
            (0, Err(PriceEstimationError::RateLimited)),
            (1, Err(PriceEstimationError::EstimatorInternal(anyhow!("")))),
            (0, Err(PriceEstimationError::ProtocolInternal(anyhow!("")))),
        ];

        for (query_index, result) in expectations {
            let expected_query = queries[query_index].clone();
            estimator
                .expect_estimate()
                .times(1)
                .withf(move |q| *q == expected_query)
                .returning(move |_| {
                    let result = result.clone();
                    async { result }.boxed()
                });
        }

        let instrumented = InstrumentedPriceEstimator::new(estimator, "foo".to_string());

        for i in 0..7 {
            let query = if i % 2 == 0 { &queries[0] } else { &queries[1] };
            let _ = instrumented.estimate(query.clone()).await;
        }

        for result in &[
            "unsupported_token",
            "unsupported_order_type",
            "rate_limited",
            "estimator_internal_error",
            "protocol_internal_error"
        ] {
            let observed = instrumented
                .metrics
                .price_estimates
                .with_label_values(&["foo", result])
                .get();
            assert_eq!(observed, 1);
        }
        let observed_success = instrumented
            .metrics
            .price_estimates
            .with_label_values(&["foo", "success"])
            .get();
        assert_eq!(observed_success, 2);
        let observed_count = instrumented
            .metrics
            .price_estimation_times
            .with_label_values(&["foo"])
            .get_sample_count();
        assert_eq!(observed_count, 7);
    }
}
