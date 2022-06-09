use crate::{
    http_client::{RateLimiter, RateLimitingStrategy},
    price_estimation::{single_estimate, PriceEstimating, PriceEstimationError, Query},
};
use futures::stream::{BoxStream, StreamExt};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub trait RateLimitedPriceEstimatorExt {
    /// Wraps an existing price estimator in rate limiting one.
    fn rate_limited(
        self,
        strategy: RateLimitingStrategy,
        healthy_response_time: Duration,
    ) -> RateLimitedPriceEstimator;
}

impl<T: PriceEstimating + 'static> RateLimitedPriceEstimatorExt for T {
    fn rate_limited(
        self,
        strategy: RateLimitingStrategy,
        healthy_response_time: Duration,
    ) -> RateLimitedPriceEstimator {
        RateLimitedPriceEstimator {
            inner: Arc::new(self),
            rate_limiter: strategy.into(),
            healthy_response_time,
        }
    }
}

/// A price estimator which backs off when it detects long response times.
/// Executes a list of queries sequentially.
pub struct RateLimitedPriceEstimator {
    inner: Arc<dyn PriceEstimating>,
    rate_limiter: RateLimiter,
    healthy_response_time: Duration,
}

impl RateLimitedPriceEstimator {
    /// Wraps an existing price estimator in rate limiting one.
    pub fn new(
        inner: Arc<dyn PriceEstimating>,
        strategy: RateLimitingStrategy,
        healthy_response_time: Duration,
    ) -> Self {
        Self {
            inner,
            rate_limiter: strategy.into(),
            healthy_response_time,
        }
    }
}

#[async_trait::async_trait]
impl PriceEstimating for RateLimitedPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> BoxStream<'_, (usize, super::PriceEstimateResult)> {
        let start = Instant::now();
        futures::stream::iter(queries.iter())
            .enumerate()
            .then(move |(index, query)| async move {
                let result = self
                    .rate_limiter
                    .execute(single_estimate(&*self.inner, query), |_| {
                        let elapsed = start.elapsed();
                        elapsed > self.healthy_response_time
                    })
                    .await
                    .map_err(PriceEstimationError::from);

                (index, result)
            })
            .boxed()
    }
}

