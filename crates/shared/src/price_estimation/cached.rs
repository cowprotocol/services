use super::{Estimate, PriceEstimating, PriceEstimationError, Query};
use cached::{Cached, TimedSizedCache};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// Price estimator wrapper that caches Ok results for some time.
pub struct CachingPriceEstimator {
    inner: Box<dyn PriceEstimating>,
    cache: Mutex<TimedSizedCache<Query, Estimate>>,
    metrics: Arc<dyn Metrics>,
    name: String,
}

#[cfg_attr(test, mockall::automock)]
pub trait Metrics: Send + Sync + 'static {
    fn price_estimator_cache(&self, name: &str, misses: usize, hits: usize);
}

struct NoopMetrics;
impl Metrics for NoopMetrics {
    fn price_estimator_cache(&self, _: &str, _: usize, _: usize) {}
}

impl CachingPriceEstimator {
    pub fn new(
        inner: Box<dyn PriceEstimating>,
        max_age: Duration,
        max_size: usize,
        metrics: Arc<dyn Metrics>,
        name: String,
    ) -> Self {
        Self {
            inner,
            cache: Mutex::new(TimedSizedCache::with_size_and_lifespan_and_refresh(
                max_size,
                max_age.as_secs(),
                false,
            )),
            metrics,
            name,
        }
    }
}

#[async_trait::async_trait]
impl PriceEstimating for CachingPriceEstimator {
    async fn estimates(&self, queries: &[Query]) -> Vec<Result<Estimate, PriceEstimationError>> {
        let mut cached: Vec<Option<Estimate>> = Vec::with_capacity(queries.len());
        let mut missing: Vec<Query> = Vec::new();

        {
            let mut cache = self.cache.lock().unwrap();
            for query in queries {
                match cache.cache_get(query) {
                    Some(estimate) => {
                        cached.push(Some(*estimate));
                    }
                    None => {
                        cached.push(None);
                        missing.push(*query);
                    }
                }
            }
        }

        self.metrics.price_estimator_cache(
            &self.name,
            missing.len(),
            queries.len() - missing.len(),
        );

        let inner_results = self.inner.estimates(&missing).await;
        {
            let mut cache = self.cache.lock().unwrap();
            for (query, result) in missing.iter().zip(inner_results.iter()) {
                if let Ok(estimate) = result {
                    cache.cache_set(*query, *estimate);
                }
            }
        }

        let mut inner_results = inner_results.into_iter();
        cached
            .into_iter()
            .map(|r| match r {
                Some(estimate) => Ok(estimate),
                // unwrap because None count == inner_results.len()
                None => inner_results.next().unwrap(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::MockPriceEstimating;
    use super::*;
    use futures::FutureExt;
    use primitive_types::H160;

    #[test]
    fn cache_is_used() {
        let query = |u: u64| Query {
            sell_token: H160::from_low_u64_be(u),
            ..Default::default()
        };
        let mut inner = MockPriceEstimating::new();
        inner.expect_estimates().times(1).returning(|queries| {
            assert!(queries.len() == 1);
            assert!(queries[0].sell_token.to_low_u64_be() == 0);
            vec![Ok(Estimate {
                out_amount: 0.into(),
                gas: 0.into(),
            })]
        });
        inner.expect_estimates().times(1).returning(|queries| {
            assert!(queries.len() == 1);
            assert!(queries[0].sell_token.to_low_u64_be() == 1);
            vec![Ok(Estimate {
                out_amount: 1.into(),
                gas: 0.into(),
            })]
        });
        let cache = CachingPriceEstimator::new(
            Box::new(inner),
            Duration::from_secs(1),
            10,
            Arc::new(NoopMetrics),
            "".to_string(),
        );
        let result = cache.estimates(&[query(0)]).now_or_never().unwrap();
        assert!(result[0].as_ref().unwrap().out_amount == 0.into());
        let result = cache
            .estimates(&[query(1), query(0)])
            .now_or_never()
            .unwrap();
        assert!(result[0].as_ref().unwrap().out_amount == 1.into());
        assert!(result[1].as_ref().unwrap().out_amount == 0.into());
    }
}
