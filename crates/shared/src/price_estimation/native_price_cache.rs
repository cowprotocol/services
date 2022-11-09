use crate::price_estimation::native::{NativePriceEstimateResult, NativePriceEstimating};
use futures::stream::{Stream, StreamExt};
use itertools::{Either, Itertools};
use primitive_types::H160;
use prometheus::IntCounterVec;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// native price cache hits misses
    #[metric(labels("result"))]
    native_price_cache: IntCounterVec,
}

#[derive(Debug, Clone)]
struct CachedPrice {
    price: f64,
    updated_at: Instant,
    requested_at: Instant,
}

struct Inner {
    cache: Mutex<HashMap<H160, CachedPrice>>,
    estimator: Box<dyn NativePriceEstimating>,
    max_age: Duration,
    metrics: &'static Metrics,
}

impl Inner {
    /// Callers should ensure tokens is not empty to prevent unnecessary call to inner price estimator.
    fn estimate_prices_and_update_cache<'a>(
        &'a self,
        tokens: &'a [H160],
    ) -> impl Stream<Item = (usize, NativePriceEstimateResult)> + 'a {
        debug_assert!(!tokens.is_empty());
        self.estimator
            .estimate_native_prices(tokens)
            .inspect(|(i, result)| {
                if let Ok(price) = result {
                    let token = &tokens[*i];
                    let now = Instant::now();
                    let mut cache = self.cache.lock().unwrap();
                    let mut entry = cache.entry(*token).or_insert_with(|| CachedPrice {
                        price: *price,
                        updated_at: now,
                        requested_at: now,
                    });
                    entry.updated_at = now;
                    entry.requested_at = now;
                    entry.price = *price;
                }
            })
    }

    /// Returns cached results and uncached indexes.
    fn get_cached_prices(&self, tokens: &[H160]) -> (Vec<(usize, f64)>, Vec<usize>) {
        if tokens.is_empty() {
            return Default::default();
        }

        let now = Instant::now();
        let mut cache = self.cache.lock().unwrap();
        tokens
            .iter()
            .enumerate()
            .partition_map(|(i, token)| match cache.get_mut(token) {
                Some(entry) if now.saturating_duration_since(entry.updated_at) < self.max_age => {
                    entry.requested_at = now;
                    Either::Left((i, entry.price))
                }
                _ => Either::Right(i),
            })
    }
}

/// Wrapper around `Box<dyn PriceEstimating>` which caches successful price estimates for some time
/// and supports updating the cache in the background.
/// The size of the underlying cache is unbounded.
pub struct CachingNativePriceEstimator(Arc<Inner>);

impl CachingNativePriceEstimator {
    /// Creates new CachingNativePriceEstimator using `estimator` to calculate native prices which
    /// get cached a duration of `max_age`.
    /// Spawns a background task maintaining the cache once per `update_interval`.
    /// Only soon to be outdated prices get updated and recently used prices have a higher priority.
    /// If `update_size` is `Some(n)` at most `n` prices get updated per interval.
    /// If `update_size` is `None` no limit gets applied.
    pub fn new(
        estimator: Box<dyn NativePriceEstimating>,
        max_age: Duration,
        update_interval: Duration,
        update_size: Option<usize>,
        prefetch_time: Option<Duration>,
    ) -> Self {
        let inner = Arc::new(Inner {
            estimator,
            cache: Default::default(),
            max_age,
            metrics: Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
        });
        tokio::spawn(update_recently_used_outdated_prices(
            Arc::downgrade(&inner),
            update_interval,
            update_size,
            prefetch_time.unwrap_or(PREFETCH_TIME),
        ));
        Self(inner)
    }
}

#[async_trait::async_trait]
impl NativePriceEstimating for CachingNativePriceEstimator {
    fn estimate_native_prices<'a>(
        &'a self,
        tokens: &'a [H160],
    ) -> futures::stream::BoxStream<'_, (usize, NativePriceEstimateResult)> {
        let stream = async_stream::stream!({
            let (cached_prices, missing_indices) = self.0.get_cached_prices(tokens);
            self.0
                .metrics
                .native_price_cache
                .with_label_values(&["misses"])
                .inc_by(missing_indices.len() as u64);
            self.0
                .metrics
                .native_price_cache
                .with_label_values(&["hits"])
                .inc_by(cached_prices.len() as u64);

            for (index, price) in cached_prices {
                yield (index, Ok(price));
            }

            if missing_indices.is_empty() {
                return;
            }
            let missing_tokens: Vec<H160> = missing_indices.iter().map(|i| tokens[*i]).collect();
            let mut stream = self.0.estimate_prices_and_update_cache(&missing_tokens);
            while let Some((i, result)) = stream.next().await {
                yield (missing_indices[i], result);
            }
        });
        stream.boxed()
    }
}

// Update prices early by this amount to increase the number of cache hits.
const PREFETCH_TIME: Duration = Duration::from_millis(2_000);

async fn update_recently_used_outdated_prices(
    inner: Weak<Inner>,
    update_interval: Duration,
    update_size: Option<usize>,
    prefetch_time: Duration,
) {
    while let Some(inner) = inner.upgrade() {
        let now = Instant::now();

        let mut outdated_entries: Vec<_> = inner
            .cache
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| {
                now.saturating_duration_since(cached.updated_at)
                    .saturating_add(prefetch_time)
                    > inner.max_age
            })
            .map(|(token, cached)| (*token, cached.requested_at))
            .collect();
        outdated_entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));

        let tokens_to_update: Vec<_> = outdated_entries
            .iter()
            .take(update_size.unwrap_or(outdated_entries.len()))
            .map(|(token, _)| *token)
            .collect();

        if !tokens_to_update.is_empty() {
            let mut stream = inner.estimate_prices_and_update_cache(&tokens_to_update);
            while stream.next().await.is_some() {}
        }

        tokio::time::sleep(update_interval.saturating_sub(now.elapsed())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::{native::MockNativePriceEstimating, PriceEstimationError};
    use num::ToPrimitive;

    fn token(u: u64) -> H160 {
        H160::from_low_u64_be(u)
    }

    #[tokio::test]
    async fn caches_successful_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                assert!(tokens[0] == token(0));
                futures::stream::iter([(0, Ok(1.0))]).boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            None,
        );

        for _ in 0..10 {
            let tokens = &[token(0)];
            let mut stream = estimator.estimate_native_prices(tokens);
            let (index, result) = stream.next().await.unwrap();
            assert_eq!(index, 0);
            assert!(result.as_ref().unwrap().to_i64().unwrap() == 1);
            assert!(stream.next().await.is_none());
        }
    }

    #[tokio::test]
    async fn does_not_cache_failed_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_prices()
            .times(10)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                futures::stream::iter([(0, Err(PriceEstimationError::NoLiquidity))]).boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            None,
        );

        for _ in 0..10 {
            let tokens = &[token(0)];
            let mut stream = estimator.estimate_native_prices(tokens);
            let (_, result) = stream.next().await.unwrap();
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::NoLiquidity
            ));
        }
    }

    #[tokio::test]
    async fn maintenance_can_limit_update_size_to_n() {
        let mut inner = MockNativePriceEstimating::new();
        // first request from user
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                assert_eq!(tokens[0], token(0));
                futures::stream::iter([(0, Ok(1.0))]).boxed()
            });
        // second request from user
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                assert_eq!(tokens[0], token(1));
                futures::stream::iter([(0, Ok(2.0))]).boxed()
            });
        // maintenance task updates n=1 outdated prices
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                assert_eq!(tokens[0], token(1));
                futures::stream::iter([(0, Ok(4.0))]).boxed()
            });
        // user requested something which has been skipped by the maintenance task
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                assert_eq!(tokens[0], token(0));
                futures::stream::iter([(0, Ok(3.0))]).boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            Some(1),
            Some(Duration::default()),
        );

        // fill cache with 2 different queries
        let results = estimator
            .estimate_native_prices(&[token(0)])
            .collect::<Vec<_>>()
            .await;
        assert!(results[0].1.as_ref().unwrap().to_i64().unwrap() == 1);
        let results = estimator
            .estimate_native_prices(&[token(1)])
            .collect::<Vec<_>>()
            .await;
        assert!(results[0].1.as_ref().unwrap().to_i64().unwrap() == 2);

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        let results = estimator
            .estimate_native_prices(&[token(0), token(1)])
            .collect::<Vec<_>>()
            .await;

        // this result has been updated in the background and therefore comes from the cache
        // the cached result is returned first
        assert_eq!(results[0].0, 1);
        assert!(results[0].1.as_ref().unwrap().to_i64().unwrap() == 4);
        // this result has been skipped during maintenance and therefore needs to be estimated by the
        // wrapped native price estimator
        assert_eq!(results[1].0, 0);
        assert!(results[1].1.as_ref().unwrap().to_i64().unwrap() == 3);
    }

    #[tokio::test]
    async fn maintenance_can_update_all_old_queries() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 10);
                futures::stream::iter(std::iter::repeat(Ok(1.0)).enumerate().take(10)).boxed()
            });
        // background task updates all outdated prices
        inner
            .expect_estimate_native_prices()
            .times(1)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 10);
                futures::stream::iter(std::iter::repeat(Ok(2.0)).enumerate().take(10)).boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            None,
            Some(Duration::default()),
        );

        let tokens: Vec<_> = (0..10).map(H160::from_low_u64_be).collect();
        let results = estimator
            .estimate_native_prices(&tokens)
            .collect::<Vec<_>>()
            .await;
        for (_, price) in &results {
            assert_eq!(price.as_ref().unwrap().to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        let results = estimator
            .estimate_native_prices(&tokens)
            .collect::<Vec<_>>()
            .await;
        for (_, price) in &results {
            assert_eq!(price.as_ref().unwrap().to_i64().unwrap(), 2);
        }
    }
}
