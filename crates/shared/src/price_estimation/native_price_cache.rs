use crate::price_estimation::native::{NativePriceEstimateResult, NativePriceEstimating};
use futures::stream::StreamExt;
use itertools::{Either, Itertools};
use primitive_types::H160;
use prometheus::{IntCounter, IntCounterVec, IntGauge};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, Weak},
    time::{Duration, Instant},
};
use tracing::Instrument;

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// native price cache hits misses
    #[metric(labels("result"))]
    native_price_cache_access: IntCounterVec,
    /// number of items in cache
    native_price_cache_size: IntGauge,
    /// number of background updates performed
    native_price_cache_background_updates: IntCounter,
    /// number of items in cache that are outdated
    native_price_cache_outdated_entries: IntGauge,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap()
    }
}

/// Wrapper around `Box<dyn PriceEstimating>` which caches successful price estimates for some time
/// and supports updating the cache in the background.
///
/// The size of the underlying cache is unbounded.
///
/// Is an Arc internally.
#[derive(Clone)]
pub struct CachingNativePriceEstimator(Arc<Inner>);

struct Inner {
    cache: Mutex<HashMap<H160, CachedPrice>>,
    high_priority: Mutex<HashSet<H160>>,
    estimator: Box<dyn NativePriceEstimating>,
    max_age: Duration,
    metrics: &'static Metrics,
}

#[derive(Debug, Clone)]
struct CachedPrice {
    price: f64,
    updated_at: Instant,
    requested_at: Instant,
}

impl Inner {
    // Returns a single cached price and updates its `requested_at` field.
    fn get_cached_price(
        token: &H160,
        now: Instant,
        cache: &mut MutexGuard<HashMap<H160, CachedPrice>>,
        max_age: &Duration,
        create_missing_entry: bool,
    ) -> Option<f64> {
        match cache.entry(*token) {
            Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.requested_at = now;
                let is_recent = now.saturating_duration_since(entry.updated_at) < *max_age;
                is_recent.then_some(entry.price)
            }
            Entry::Vacant(entry) => {
                if create_missing_entry {
                    // Create an outdated cache entry so the background task keeping the cache warm
                    // will fetch the price during the next maintenance cycle.
                    // This should happen only for prices missing while building the auction.
                    // Otherwise malicious actors could easily cause the cache size to blow up.
                    let outdated_timestamp = now.checked_sub(*max_age).unwrap();
                    entry.insert(CachedPrice {
                        price: 0.,
                        updated_at: outdated_timestamp,
                        requested_at: now,
                    });
                }
                None
            }
        }
    }

    /// Returns cached prices and uncached indices.
    fn get_cached_prices(
        &self,
        tokens: &[H160],
        max_age: &Duration,
        create_missing_entry: bool,
    ) -> (Vec<(usize, f64)>, Vec<usize>) {
        if tokens.is_empty() {
            return Default::default();
        }

        let now = Instant::now();
        let mut cache = self.cache.lock().unwrap();
        tokens.iter().enumerate().partition_map(|(i, token)| {
            match Self::get_cached_price(token, now, &mut cache, max_age, create_missing_entry) {
                Some(price) => Either::Left((i, price)),
                _ => Either::Right(i),
            }
        })
    }

    /// Checks cache for the given tokens one by one. If the price is already cached it gets
    /// returned. If it's not in the cache a new price estimation request gets issued.
    /// We check the cache before each request because they can take a long time and some other
    /// task might have fetched some requested price in the meantime.
    fn estimate_prices_and_update_cache<'a>(
        &'a self,
        tokens: &'a [H160],
        max_age: Duration,
        parallelism: usize,
    ) -> futures::stream::BoxStream<'_, (usize, NativePriceEstimateResult)> {
        let estimates = tokens
            .iter()
            .enumerate()
            .map(move |(index, token)| async move {
                {
                    // check if price is cached by now
                    let now = Instant::now();
                    let mut cache = self.cache.lock().unwrap();
                    let price = Self::get_cached_price(token, now, &mut cache, &max_age, false);
                    if let Some(price) = price {
                        return (index, Ok(price));
                    }
                }

                // fetch single price estimate
                let (_, result) = self
                    .estimator
                    .estimate_native_prices(&[*token])
                    .next()
                    .await
                    .expect("stream yields exactly 1 item");

                // update price in cache
                if let Ok(price) = result {
                    let now = Instant::now();
                    let mut cache = self.cache.lock().unwrap();
                    cache.insert(
                        *token,
                        CachedPrice {
                            price,
                            updated_at: now,
                            requested_at: now,
                        },
                    );
                };

                (index, result)
            });
        futures::stream::iter(estimates)
            .buffered(parallelism)
            .boxed()
    }

    /// Tokens with highest priority first.
    fn sorted_tokens_to_update(&self, max_age: Duration, now: Instant) -> Vec<(H160, Instant)> {
        let mut outdated: Vec<_> = self
            .cache
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > max_age)
            .map(|(token, cached)| (*token, cached.requested_at))
            .collect();
        let high_priority = self.high_priority.lock().unwrap().clone();
        let priority = |token: &H160| high_priority.contains(token) as u8;
        outdated.sort_unstable_by_key(|entry| {
            (
                std::cmp::Reverse(priority(&entry.0)),
                std::cmp::Reverse(entry.1),
            )
        });
        outdated
    }
}

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
        prefetch_time: Duration,
        concurrent_requests: usize,
    ) -> Self {
        let inner = Arc::new(Inner {
            estimator,
            cache: Default::default(),
            high_priority: Default::default(),
            max_age,
            metrics: Metrics::get(),
        });
        tokio::spawn(
            update_recently_used_outdated_prices(
                Arc::downgrade(&inner),
                update_interval,
                update_size,
                max_age.saturating_sub(prefetch_time),
                concurrent_requests,
            )
            .instrument(tracing::info_span!("caching_native_price_estimator")),
        );
        Self(inner)
    }

    /// Only returns prices that are currently cached. Missing prices will get prioritized to get
    /// fetched during the next cycles of the maintenance background task.
    pub fn get_cached_prices(&self, tokens: &[H160]) -> HashMap<H160, f64> {
        let (cached_prices, missing_indices) =
            self.0.get_cached_prices(tokens, &self.0.max_age, true);
        self.0
            .metrics
            .native_price_cache_access
            .with_label_values(&["misses"])
            .inc_by(missing_indices.len() as u64);
        let result = cached_prices
            .iter()
            .map(|(index, price)| (tokens[*index], *price))
            .collect();
        result
    }

    pub fn replace_high_priority(&self, tokens: HashSet<H160>) {
        *self.0.high_priority.lock().unwrap() = tokens;
    }
}

#[async_trait::async_trait]
impl NativePriceEstimating for CachingNativePriceEstimator {
    fn estimate_native_prices<'a>(
        &'a self,
        tokens: &'a [H160],
    ) -> futures::stream::BoxStream<'_, (usize, NativePriceEstimateResult)> {
        let stream = async_stream::stream!({
            let (cached_prices, missing_indices) =
                self.0.get_cached_prices(tokens, &self.0.max_age, false);
            self.0
                .metrics
                .native_price_cache_access
                .with_label_values(&["misses"])
                .inc_by(missing_indices.len() as u64);
            self.0
                .metrics
                .native_price_cache_access
                .with_label_values(&["hits"])
                .inc_by(cached_prices.len() as u64);

            for (index, price) in cached_prices {
                yield (index, Ok(price));
            }

            if missing_indices.is_empty() {
                return;
            }
            let missing_tokens: Vec<H160> = missing_indices.iter().map(|i| tokens[*i]).collect();
            let mut stream =
                self.0
                    .estimate_prices_and_update_cache(&missing_tokens, self.0.max_age, 1);
            while let Some((i, result)) = stream.next().await {
                yield (missing_indices[i], result);
            }
        });
        stream.boxed()
    }
}

async fn update_recently_used_outdated_prices(
    inner: Weak<Inner>,
    update_interval: Duration,
    update_size: Option<usize>,
    max_age: Duration,
    concurrent_requests: usize,
) {
    while let Some(inner) = inner.upgrade() {
        let now = Instant::now();
        inner
            .metrics
            .native_price_cache_size
            .set(inner.cache.lock().unwrap().len() as i64);
        let outdated_entries = inner.sorted_tokens_to_update(max_age, now);
        inner
            .metrics
            .native_price_cache_outdated_entries
            .set(outdated_entries.len() as i64);
        let tokens_to_update: Vec<_> = outdated_entries
            .iter()
            .take(update_size.unwrap_or(outdated_entries.len()))
            .map(|(token, _)| *token)
            .collect();

        if !tokens_to_update.is_empty() {
            let mut stream = inner.estimate_prices_and_update_cache(
                &tokens_to_update,
                max_age,
                concurrent_requests,
            );
            while stream.next().await.is_some() {}
            inner
                .metrics
                .native_price_cache_background_updates
                .inc_by(tokens_to_update.len() as u64);
        }

        tokio::time::sleep(update_interval.saturating_sub(now.elapsed())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_estimation::{native::MockNativePriceEstimating, PriceEstimationError};
    use futures::FutureExt;
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
            Default::default(),
            1,
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
            Default::default(),
            1,
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
            Duration::default(),
            1,
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
            .times(10)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                futures::stream::iter(std::iter::once(Ok(1.0)).enumerate()).boxed()
            });
        // background task updates all outdated prices
        inner
            .expect_estimate_native_prices()
            .times(10)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                futures::stream::iter(std::iter::once(Ok(2.0)).enumerate()).boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            None,
            Duration::default(),
            1,
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

    #[tokio::test]
    async fn maintenance_can_update_concurrently() {
        const WAIT_TIME_MS: u64 = 100;
        const BATCH_SIZE: usize = 100;
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_prices()
            .times(BATCH_SIZE)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                futures::stream::iter(std::iter::once(Ok(1.0)).enumerate()).boxed()
            });
        // background task updates all outdated prices
        inner
            .expect_estimate_native_prices()
            .times(BATCH_SIZE)
            .returning(move |tokens| {
                assert_eq!(tokens.len(), 1);
                async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(WAIT_TIME_MS)).await;
                    (0, Ok(2.0))
                }
                .into_stream()
                .boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            None,
            Duration::default(),
            BATCH_SIZE,
        );

        let tokens: Vec<_> = (0..BATCH_SIZE as u64).map(H160::from_low_u64_be).collect();
        let results = estimator
            .estimate_native_prices(&tokens)
            .collect::<Vec<_>>()
            .await;
        for (_, price) in &results {
            assert_eq!(price.as_ref().unwrap().to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        // although we have 100 requests which all take 100ms to complete the maintenance cycle
        // completes sooner because all requests are handled concurrently.
        tokio::time::sleep(Duration::from_millis(60 + WAIT_TIME_MS)).await;

        let results = estimator
            .estimate_native_prices(&tokens)
            .collect::<Vec<_>>()
            .await;
        for (_, price) in &results {
            assert_eq!(price.as_ref().unwrap().to_i64().unwrap(), 2);
        }
    }

    #[test]
    fn outdated_entries_prioritized() {
        let t0 = H160::from_low_u64_be(0);
        let t1 = H160::from_low_u64_be(1);
        let now = Instant::now();
        let inner = Inner {
            cache: Mutex::new(
                [
                    (
                        t0,
                        CachedPrice {
                            price: 0.,
                            updated_at: now,
                            requested_at: now,
                        },
                    ),
                    (
                        t1,
                        CachedPrice {
                            price: 0.,
                            updated_at: now,
                            requested_at: now,
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            high_priority: Default::default(),
            estimator: Box::new(MockNativePriceEstimating::new()),
            max_age: Default::default(),
            metrics: Metrics::get(),
        };

        let now = now + Duration::from_secs(1);

        *inner.high_priority.lock().unwrap() = std::iter::once(t0).collect();
        let tokens = inner.sorted_tokens_to_update(Duration::from_secs(0), now);
        assert_eq!(tokens[0].0, t0);
        assert_eq!(tokens[1].0, t1);

        *inner.high_priority.lock().unwrap() = std::iter::once(t1).collect();
        let tokens = inner.sorted_tokens_to_update(Duration::from_secs(0), now);
        assert_eq!(tokens[0].0, t1);
        assert_eq!(tokens[1].0, t0);
    }
}
