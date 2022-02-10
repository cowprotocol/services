use crate::price_estimation::native::NativePriceEstimating;
use crate::price_estimation::PriceEstimationError;
use primitive_types::H160;
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};

#[cfg_attr(test, mockall::automock)]
pub trait Metrics: Send + Sync + 'static {
    fn native_price_cache(&self, misses: usize, hits: usize);
}

struct NoopMetrics;
impl Metrics for NoopMetrics {
    fn native_price_cache(&self, _: usize, _: usize) {}
}

#[derive(Debug, Clone)]
struct CachedPrice {
    price: f64,
    updated_at: Instant,
}

struct Inner {
    cache: RwLock<HashMap<H160, CachedPrice>>,
    estimator: Box<dyn NativePriceEstimating>,
    max_age: Duration,
    metrics: Arc<dyn Metrics>,
}

impl Inner {
    async fn estimate_prices_and_update_cache(
        &self,
        tokens: &[H160],
    ) -> Vec<Result<f64, PriceEstimationError>> {
        if tokens.is_empty() {
            return Vec::default();
        }

        let now = Instant::now();
        let prices = self.estimator.estimate_native_prices(tokens).await;
        {
            let mut cache = self.cache.write().unwrap();
            for (token, result) in tokens.iter().zip(prices.iter()) {
                if let Ok(price) = result {
                    let mut entry = cache.entry(*token).or_insert_with(|| CachedPrice {
                        price: *price,
                        updated_at: now,
                    });
                    entry.updated_at = now;
                    entry.price = *price;
                }
            }
        }
        prices
    }

    fn get_cached_prices(&self, tokens: &[H160]) -> Vec<Option<f64>> {
        if tokens.is_empty() {
            return Vec::default();
        }

        let now = Instant::now();
        let cache = self.cache.read().unwrap();
        tokens
            .iter()
            .map(|token| match cache.get(token) {
                Some(entry) if now.saturating_duration_since(entry.updated_at) < self.max_age => {
                    Some(entry.price)
                }
                _ => None,
            })
            .collect()
    }
}

/// Wrapper around `Box<dyn PriceEstimating>` which caches successful price estimates for some time
/// and supports updating the cache in the background.
/// The size of the underlying cache is unbounded.
pub struct CachingNativePriceEstimator(Arc<Inner>);

impl CachingNativePriceEstimator {
    /// Creates new CachingNativePriceEstimator using `estimator` to calculate native prices which
    /// get cached a duration of `max_age`.
    pub fn new(
        estimator: Box<dyn NativePriceEstimating>,
        max_age: Duration,
        metrics: Arc<dyn Metrics>,
    ) -> Self {
        Self(Arc::new(Inner {
            estimator,
            cache: RwLock::new(Default::default()),
            max_age,
            metrics,
        }))
    }

    /// Spawns a background task maintaining the cache once per `update_interval`.
    /// Only outdated prices get updated and older prices have a higher priority.
    /// If `update_size` is `Some(n)` at most `n` prices get updated per interval.
    /// If `update_size` is `None` no limit gets applied.
    pub fn spawn_maintenance_task(&self, update_interval: Duration, update_size: Option<usize>) {
        tokio::spawn(update_most_outdated_prices(
            Arc::downgrade(&self.0),
            update_interval,
            update_size,
        ));
    }
}

#[async_trait::async_trait]
impl NativePriceEstimating for CachingNativePriceEstimator {
    async fn estimate_native_prices(
        &self,
        tokens: &[H160],
    ) -> Vec<Result<f64, PriceEstimationError>> {
        let cached_prices = self.0.get_cached_prices(tokens);

        let missing_tokens: Vec<_> = tokens
            .iter()
            .zip(cached_prices.iter())
            .filter_map(|(token, price)| match price {
                Some(_) => None,
                None => Some(*token),
            })
            .collect();

        self.0.metrics.native_price_cache(
            missing_tokens.len(),
            cached_prices.len() - missing_tokens.len(),
        );

        let mut results = self
            .0
            .estimate_prices_and_update_cache(&missing_tokens)
            .await
            .into_iter();

        cached_prices
            .into_iter()
            .map(|r| match r {
                Some(estimate) => Ok(estimate),
                // unwrap because None count == inner_results.len()
                None => results.next().unwrap(),
            })
            .collect()
    }
}

async fn update_most_outdated_prices(
    inner: Weak<Inner>,
    update_interval: Duration,
    update_size: Option<usize>,
) {
    while let Some(inner) = inner.upgrade() {
        let now = Instant::now();

        let mut outdated_entries: Vec<_> = inner
            .cache
            .read()
            .unwrap()
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > inner.max_age)
            .map(|(token, cached)| (*token, cached.updated_at))
            .collect();
        outdated_entries.sort_by_key(|entry| entry.1);

        let tokens_to_update: Vec<_> = outdated_entries
            .iter()
            .take(update_size.unwrap_or(outdated_entries.len()))
            .map(|(token, _)| *token)
            .collect();

        inner
            .estimate_prices_and_update_cache(&tokens_to_update)
            .await;

        tokio::time::sleep(update_interval.saturating_sub(now.elapsed())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::native::MockNativePriceEstimating;
    use num::ToPrimitive;

    #[tokio::test]
    async fn caches_successful_estimates() {
        let token = H160::from_low_u64_be;
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert!(tokens.len() == 1);
                assert!(tokens[0] == token(0));
                vec![Ok(1.0)]
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Arc::new(NoopMetrics),
        );

        for _ in 0..10 {
            let results = estimator.estimate_native_prices(&[token(0)]).await;
            assert!(results[0].as_ref().unwrap().to_i64().unwrap() == 1);
        }
    }

    #[tokio::test]
    async fn does_not_cache_failed_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_prices()
            .times(10)
            .returning(move |tokens| {
                assert!(tokens.len() == 1);
                vec![Err(PriceEstimationError::NoLiquidity)]
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Arc::new(NoopMetrics),
        );

        for _ in 0..10 {
            let results = estimator
                .estimate_native_prices(&[H160::from_low_u64_be(0)])
                .await;
            assert!(matches!(
                results[0].as_ref().unwrap_err(),
                PriceEstimationError::NoLiquidity
            ));
        }
    }

    #[tokio::test]
    async fn maintenance_can_limit_update_size_to_n() {
        let token = H160::from_low_u64_be;
        let mut inner = MockNativePriceEstimating::new();
        // first request from user
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert!(tokens.len() == 1);
                assert!(tokens[0] == token(0));
                vec![Ok(1.0)]
            });
        // second request from user
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert!(tokens.len() == 1);
                assert!(tokens[0] == token(1));
                vec![Ok(2.0)]
            });
        // maintenance task updates n=1 outdated prices
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert!(tokens.len() == 1);
                assert!(tokens[0] == token(0));
                vec![Ok(3.0)]
            });
        // user requested something which has been skipped by the maintenance task
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert!(tokens.len() == 1);
                assert!(tokens[0] == token(1));
                vec![Ok(4.0)]
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Arc::new(NoopMetrics),
        );
        estimator.spawn_maintenance_task(Duration::from_millis(50), Some(1));

        // fill cache with 2 different queries
        let results = estimator.estimate_native_prices(&[token(0)]).await;
        assert!(results[0].as_ref().unwrap().to_i64().unwrap() == 1);
        let results = estimator.estimate_native_prices(&[token(1)]).await;
        assert!(results[0].as_ref().unwrap().to_i64().unwrap() == 2);

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        let results = estimator
            .estimate_native_prices(&[token(0), token(1)])
            .await;

        // this result has been updated in the background and therefore comes from the cache
        assert!(results[0].as_ref().unwrap().to_i64().unwrap() == 3);
        // this result has been skipped during maintenance and therefore needs to be estimated by the
        // wrapped native price estimator
        assert!(results[1].as_ref().unwrap().to_i64().unwrap() == 4);
    }

    #[tokio::test]
    async fn maintenance_can_update_all_old_queries() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert!(tokens.len() == 10);
                vec![Ok(1.0); 10]
            });
        // background task updates all outdated prices
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert!(tokens.len() == 10);
                vec![Ok(2.0); 10]
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Arc::new(NoopMetrics),
        );
        estimator.spawn_maintenance_task(Duration::from_millis(50), None);

        let tokens: Vec<_> = (0..10).map(H160::from_low_u64_be).collect();
        let results = estimator.estimate_native_prices(&tokens).await;
        for price in &results {
            assert_eq!(price.as_ref().unwrap().to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        let results = estimator.estimate_native_prices(&tokens).await;
        for price in &results {
            assert_eq!(price.as_ref().unwrap().to_i64().unwrap(), 2);
        }
    }
}
