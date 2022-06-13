use crate::{
    price_estimation::{single_estimate, PriceEstimating, PriceEstimationError, Query},
    rate_limiter::RateLimiter,
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
        rate_limiter: RateLimiter,
        healthy_response_time: Duration,
    ) -> RateLimitedPriceEstimator;
}

impl<T: PriceEstimating + 'static> RateLimitedPriceEstimatorExt for T {
    fn rate_limited(
        self,
        rate_limiter: RateLimiter,
        healthy_response_time: Duration,
    ) -> RateLimitedPriceEstimator {
        RateLimitedPriceEstimator {
            inner: Arc::new(self),
            rate_limiter,
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
        rate_limiter: RateLimiter,
        healthy_response_time: Duration,
    ) -> Self {
        Self {
            inner,
            rate_limiter,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        price_estimation::{Estimate, MockPriceEstimating, PriceEstimateResult},
        rate_limiter::RateLimitingStrategy,
    };
    use futures::{FutureExt, StreamExt};
    use primitive_types::H160;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn backs_off_after_slow_response() {
        const MAX_BACK_OFF: Duration = Duration::from_millis(200);
        const HEALTHY_RESPONSE_TIME: Duration = Duration::from_millis(10);

        fn estimate(amount: u64) -> Estimate {
            Estimate {
                out_amount: amount.into(),
                ..Default::default()
            }
        }

        fn is_rate_limit_error(result: &PriceEstimateResult) -> bool {
            matches!(result, Err(e) if e.to_string().contains("backing off rate limit"))
        }

        fn query(sell_token: u8) -> Query {
            Query {
                sell_token: H160([sell_token; 20]),
                ..Default::default()
            }
        }

        let mut inner = MockPriceEstimating::new();
        inner.expect_estimates().returning(move |queries| {
            // RateLimitedPriceEstimator queries 1 price at a time
            assert_eq!(queries.len(), 1);
            let return_after_unhealthy_time = queries[0] == query(1);
            futures::stream::once(async move {
                if return_after_unhealthy_time {
                    sleep(HEALTHY_RESPONSE_TIME * 2).await;
                    (0, Ok(estimate(0)))
                } else {
                    (0, Ok(estimate(1)))
                }
            })
            .boxed()
        });

        let strategy = RateLimitingStrategy::try_new(2.0, MAX_BACK_OFF, MAX_BACK_OFF).unwrap();
        let estimator = inner.rate_limited(
            RateLimiter::from_strategy(strategy, "rate_limited".into()),
            HEALTHY_RESPONSE_TIME,
        );

        let queries = &[query(1), query(2)];

        // second query will return rate limited error
        let results: Vec<_> = estimator.estimates(queries).collect().await;
        assert_eq!(2, results.len());
        assert!(results[0].1.is_ok());
        // rate limits can start happening within one stream of queries
        assert!(is_rate_limit_error(&results[1].1));

        let results: Vec<_> = estimator
            .estimates(queries)
            .collect()
            .now_or_never()
            .expect("requests should return errors immediately while being rate limited");
        assert_eq!(2, results.len());
        assert!(is_rate_limit_error(&results[0].1));
        assert!(is_rate_limit_error(&results[1].1));

        sleep(MAX_BACK_OFF).await;
        let results: Vec<_> = estimator.estimates(queries).collect().await;
        assert_eq!(2, results.len());
        assert!(results[0].1.is_ok());
        assert!(is_rate_limit_error(&results[1].1));
    }
}
