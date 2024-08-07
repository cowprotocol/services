use {
    super::PriceEstimationError,
    crate::price_estimation::native::{NativePriceEstimateResult, NativePriceEstimating},
    anyhow::anyhow,
    async_trait::async_trait,
    futures::{FutureExt, StreamExt},
    indexmap::IndexSet,
    primitive_types::H160,
    prometheus::{IntCounter, IntCounterVec, IntGauge},
    std::{
        collections::{hash_map::Entry, HashMap, HashSet},
        sync::{Arc, Mutex, Weak},
        time::{Duration, Instant},
    },
    tokio::{
        sync::{mpsc, Mutex as TokioMutex, MutexGuard},
        time::{interval, sleep},
    },
    tracing::Instrument,
};

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

/// Trait for fetching a batch of native price estimates.
#[mockall::automock]
#[async_trait]
pub trait NativePriceBatchFetcher: Sync + Send {
    /// Maximum batch size the `fetch_native_prices()` can take
    fn max_batch_size(&self) -> usize;

    /// Fetches a batch of native price estimates. It fetches a maximum of
    /// `max_batch_size()` elements
    ///
    /// It returns a HashMap which maps the token with its price
    async fn fetch_native_prices(
        &self,
        tokens: &HashSet<H160>,
    ) -> Result<HashMap<H160, f64>, PriceEstimationError>;
}

struct Batching {
    period: Duration,
    cache: Arc<TokioMutex<HashMap<H160, CachedResult>>>,
    sender: mpsc::UnboundedSender<H160>,
}

impl Batching {
    /// Maximum number of tries for fetching the price from the cache
    const MAX_TRIES: usize = 3;

    fn new(
        cache: Arc<TokioMutex<HashMap<H160, CachedResult>>>,
        period: Duration,
        fetcher: Arc<dyn NativePriceBatchFetcher>,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel::<H160>();

        tokio::task::spawn(Self::run(receiver, cache.clone(), fetcher, period));

        Self {
            period,
            cache,
            sender,
        }
    }

    async fn run(
        mut receiver: mpsc::UnboundedReceiver<H160>,
        cache: Arc<TokioMutex<HashMap<H160, CachedResult>>>,
        fetcher: Arc<dyn NativePriceBatchFetcher>,
        period: Duration,
    ) {
        let mut interval = interval(period);
        loop {
            // Wait for the internal tick
            interval.tick().await;

            let mut tokens = HashSet::new();

            // Collect up to `MAX_BATCH_SIZE` tokens from the receiver
            for _ in 0..fetcher.max_batch_size() {
                match receiver.try_recv() {
                    Ok(token) => {
                        tokens.insert(token);
                    }
                    // No more messages to process
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        tracing::error!("native price batching: disconnected");
                        continue;
                    }
                }
            }

            if !tokens.is_empty() {
                // Call the fetcher with the collected addresses
                match fetcher.fetch_native_prices(&tokens).await {
                    Ok(results) => {
                        let now = Instant::now();
                        // Update the cache with the fetched results
                        let mut cache_lock = cache.lock().await;
                        for (address, result) in results {
                            let cached_result = CachedResult {
                                result: Ok(result),
                                updated_at: now,
                                requested_at: now,
                            };
                            cache_lock
                                .entry(address)
                                .and_modify(|entry| *entry = cached_result.clone())
                                .or_insert(cached_result);
                        }
                    }
                    Err(e) => {
                        tracing::error!(?e, "failed to fetch native prices");
                        continue;
                    }
                }
            }
        }
    }

    async fn request_native_price(&self, token: &H160) -> Result<(), PriceEstimationError> {
        self.sender.send(*token).map_err(|e| {
            PriceEstimationError::ProtocolInternal(anyhow!(
                "failed to append a new token to the queue: {e:?}"
            ))
        })
    }

    async fn blocking_estimate_prices_and_update_cache(
        &self,
        token: &H160,
    ) -> Result<Option<f64>, PriceEstimationError> {
        // Sends the token for requesting price
        self.request_native_price(token).await?;

        for _ in 0..Self::MAX_TRIES {
            // Check if the value is already in the cache
            {
                if let Some(price) = self.cache.lock().await.get(token) {
                    return price.result.clone().map(Some);
                }
            }
            sleep(self.period).await;
        }

        Ok(None)
    }
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

/// Wrapper around `Box<dyn PriceEstimating>` which caches successful price
/// estimates for some time and supports updating the cache in the background.
///
/// The size of the underlying cache is unbounded.
///
/// Is an Arc internally.
#[derive(Clone)]
pub struct CachingNativePriceEstimator(Arc<Inner>);

struct Inner {
    cache: Arc<TokioMutex<HashMap<H160, CachedResult>>>,
    high_priority: Mutex<IndexSet<H160>>,
    estimator: Box<dyn NativePriceEstimating>,
    max_age: Duration,
    batching: Option<Batching>,
}

struct UpdateTask {
    inner: Weak<Inner>,
    update_interval: Duration,
    update_size: Option<usize>,
    prefetch_time: Duration,
    concurrent_requests: usize,
}

type CacheEntry = Result<f64, PriceEstimationError>;

#[derive(Debug, Clone)]
struct CachedResult {
    result: CacheEntry,
    updated_at: Instant,
    requested_at: Instant,
}

impl Inner {
    // Returns a single cached price and updates its `requested_at` field.
    fn get_cached_price(
        token: H160,
        now: Instant,
        cache: &mut MutexGuard<HashMap<H160, CachedResult>>,
        max_age: &Duration,
        create_missing_entry: bool,
    ) -> Option<CacheEntry> {
        match cache.entry(token) {
            Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.requested_at = now;
                let is_recent = now.saturating_duration_since(entry.updated_at) < *max_age;
                is_recent.then_some(entry.result.clone())
            }
            Entry::Vacant(entry) => {
                if create_missing_entry {
                    // Create an outdated cache entry so the background task keeping the cache warm
                    // will fetch the price during the next maintenance cycle.
                    // This should happen only for prices missing while building the auction.
                    // Otherwise malicious actors could easily cause the cache size to blow up.
                    let outdated_timestamp = now.checked_sub(*max_age).unwrap();
                    entry.insert(CachedResult {
                        result: Ok(0.),
                        updated_at: outdated_timestamp,
                        requested_at: now,
                    });
                }
                None
            }
        }
    }

    async fn blocking_estimate_prices_and_update_cache(
        &self,
        token: &H160,
        max_age: Duration,
    ) -> Result<Option<f64>, PriceEstimationError> {
        if let Some(batching) = self.batching.as_ref() {
            batching
                .blocking_estimate_prices_and_update_cache(token)
                .await
        } else {
            // If the batching is not configured, use the legacy method
            self.estimate_prices_and_update_cache(&[*token], max_age, 1)
                .next()
                .await
                .map(|(_, price)| price)
                .transpose()
        }
    }

    /// Checks cache for the given tokens one by one. If the price is already
    /// cached it gets returned. If it's not in the cache a new price
    /// estimation request gets issued. We check the cache before each
    /// request because they can take a long time and some other task might
    /// have fetched some requested price in the meantime.
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
                    let mut cache = self.cache.lock().await;
                    let price = Self::get_cached_price(*token, now, &mut cache, &max_age, false);
                    if let Some(price) = price {
                        return (index, price);
                    }
                }

                let result = self.estimator.estimate_native_price(*token).await;

                // update price in cache
                if should_cache(&result) {
                    let now = Instant::now();
                    let mut cache = self.cache.lock().await;
                    cache.insert(
                        *token,
                        CachedResult {
                            result: result.clone(),
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
    async fn sorted_tokens_to_update(&self, max_age: Duration, now: Instant) -> Vec<H160> {
        let mut outdated: Vec<_> = self
            .cache
            .lock()
            .await
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > max_age)
            .map(|(token, cached)| (*token, cached.requested_at))
            .collect();

        let high_priority = self.high_priority.lock().unwrap().clone();
        let index = |token: &H160| high_priority.get_index_of(token).unwrap_or(usize::MAX);
        outdated.sort_by_cached_key(|entry| {
            (
                index(&entry.0),            // important items have a low index
                std::cmp::Reverse(entry.1), // important items have recent (i.e. "big") timestamp
            )
        });
        outdated.into_iter().map(|(token, _)| token).collect()
    }
}

fn should_cache(result: &Result<f64, PriceEstimationError>) -> bool {
    // We don't want to cache errors that we consider transient
    match result {
        Ok(_)
        | Err(PriceEstimationError::NoLiquidity { .. })
        | Err(PriceEstimationError::UnsupportedToken { .. }) => true,
        Err(PriceEstimationError::EstimatorInternal(_))
        | Err(PriceEstimationError::ProtocolInternal(_))
        | Err(PriceEstimationError::RateLimited) => false,
        Err(PriceEstimationError::UnsupportedOrderType(_)) => {
            tracing::error!(?result, "Unexpected error in native price cache");
            false
        }
    }
}

impl UpdateTask {
    /// Single run of the background updating process.
    async fn single_update(&self, inner: &Inner) {
        let metrics = Metrics::get();
        metrics
            .native_price_cache_size
            .set(i64::try_from(inner.cache.lock().await.len()).unwrap_or(i64::MAX));

        let max_age = inner.max_age.saturating_sub(self.prefetch_time);
        let mut outdated_entries = inner.sorted_tokens_to_update(max_age, Instant::now()).await;

        metrics
            .native_price_cache_outdated_entries
            .set(i64::try_from(outdated_entries.len()).unwrap_or(i64::MAX));

        outdated_entries.truncate(self.update_size.unwrap_or(usize::MAX));

        if !outdated_entries.is_empty() {
            let mut stream = inner.estimate_prices_and_update_cache(
                &outdated_entries,
                max_age,
                self.concurrent_requests,
            );
            while stream.next().await.is_some() {}
            metrics
                .native_price_cache_background_updates
                .inc_by(outdated_entries.len() as u64);
        }
    }

    /// Runs background updates until inner is no longer alive.
    async fn run(self) {
        while let Some(inner) = self.inner.upgrade() {
            let now = Instant::now();
            self.single_update(&inner).await;
            tokio::time::sleep(self.update_interval.saturating_sub(now.elapsed())).await;
        }
    }
}

impl CachingNativePriceEstimator {
    /// Creates new CachingNativePriceEstimator using `estimator` to calculate
    /// native prices which get cached a duration of `max_age`.
    /// Spawns a background task maintaining the cache once per
    /// `update_interval`. Only soon to be outdated prices get updated and
    /// recently used prices have a higher priority. If `update_size` is
    /// `Some(n)` at most `n` prices get updated per interval.
    /// If `update_size` is `None` no limit gets applied.
    pub fn new(
        estimator: Box<dyn NativePriceEstimating>,
        max_age: Duration,
        update_interval: Duration,
        update_size: Option<usize>,
        prefetch_time: Duration,
        concurrent_requests: usize,
        fetcher: Option<Arc<dyn NativePriceBatchFetcher>>,
    ) -> Self {
        let cache: Arc<TokioMutex<HashMap<H160, CachedResult>>> = Default::default();
        let inner = Arc::new(Inner {
            estimator,
            cache: cache.clone(),
            high_priority: Default::default(),
            max_age,
            batching: fetcher
                .map(|fetcher| Batching::new(cache.clone(), Duration::from_millis(50), fetcher)),
        });

        let update_task = UpdateTask {
            inner: Arc::downgrade(&inner),
            update_interval,
            update_size,
            prefetch_time,
            concurrent_requests,
        }
        .run()
        .instrument(tracing::info_span!("caching_native_price_estimator"));
        tokio::spawn(update_task);

        Self(inner)
    }

    /// Only returns prices that are currently cached. Missing prices will get
    /// prioritized to get fetched during the next cycles of the maintenance
    /// background task.
    pub async fn get_cached_prices(
        &self,
        tokens: &[H160],
    ) -> HashMap<H160, Result<f64, PriceEstimationError>> {
        let now = Instant::now();
        let mut cache = self.0.cache.lock().await;
        let mut results = HashMap::default();
        for token in tokens {
            let cached = Inner::get_cached_price(*token, now, &mut cache, &self.0.max_age, true);
            let label = if cached.is_some() { "hits" } else { "misses" };
            Metrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);
            if let Some(result) = cached {
                results.insert(*token, result);
            }
        }
        results
    }

    pub fn replace_high_priority(&self, tokens: IndexSet<H160>) {
        *self.0.high_priority.lock().unwrap() = tokens;
    }
}

impl NativePriceEstimating for CachingNativePriceEstimator {
    fn estimate_native_price(
        &self,
        token: H160,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let cached = {
                let now = Instant::now();
                let mut cache = self.0.cache.lock().await;
                Inner::get_cached_price(token, now, &mut cache, &self.0.max_age, false)
            };

            let label = if cached.is_some() { "hits" } else { "misses" };
            Metrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);

            if let Some(price) = cached {
                return price;
            }

            self.0
                .blocking_estimate_prices_and_update_cache(&token, self.0.max_age)
                .await?
                .ok_or(PriceEstimationError::NoLiquidity)
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::{
            native::{MockNativePriceEstimating, NativePriceEstimating},
            PriceEstimationError,
        },
        futures::{future::try_join_all, FutureExt},
        num::ToPrimitive,
    };

    fn token(u: u64) -> H160 {
        H160::from_low_u64_be(u)
    }

    #[tokio::test]
    async fn caches_from_batch_request_successful_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            // Because it gets the value from the batch estimator, it does not need to do this call at all
            .never();

        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one, because for the second call it fetches the cached one
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });
        let fetcher: Arc<dyn NativePriceBatchFetcher> = Arc::new(native_price_batch_fetcher);

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            // The maintenance cache updater runs every 250 ms
            Duration::from_millis(250),
            Default::default(),
            None,
            Default::default(),
            1,
            Some(fetcher),
        );

        // We wait some time so the maintenance has run at least one
        sleep(Duration::from_millis(100)).await;

        for _ in 0..10 {
            // Launch requests and see we get the response `1` from the native batch fetcher instead of the one from the maintenance job
            let result = estimator.estimate_native_price(token(0)).await;
            assert!(result.as_ref().unwrap().to_i64().unwrap() == 1);
        }
    }

    #[tokio::test]
    async fn caches_successful_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|_| async { Ok(1.0) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            None,
        );

        for _ in 0..10 {
            let result = estimator.estimate_native_price(token(0)).await;
            assert!(result.as_ref().unwrap().to_i64().unwrap() == 1);
        }
    }

    #[tokio::test]
    async fn batching_successful_estimates() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one, because for the second call it fetches the cached one
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });
        let cache: Arc<TokioMutex<HashMap<H160, CachedResult>>> = Default::default();
        let batching = Batching::new(
            cache.clone(),
            Duration::from_nanos(10),
            Arc::new(native_price_batch_fetcher),
        );

        let result = batching
            .blocking_estimate_prices_and_update_cache(&token(0))
            .await
            .expect("valid result")
            .unwrap();

        assert_eq!(result.to_i64().unwrap(), 1);
        let cached_price = cache.lock().await.get(&token(0)).unwrap().clone();
        // Check the cache was updated successfully
        assert_eq!(cached_price.result.unwrap().to_i64().unwrap(), 1);

        // Another request, but since the value is cached, it should not   request
        // `fetch_native_prices()`
        let result = batching
            .blocking_estimate_prices_and_update_cache(&token(0))
            .await
            .expect("valid result")
            .unwrap();

        assert_eq!(result.to_i64().unwrap(), 1);
        let cached_price = cache.lock().await.get(&token(0)).unwrap().clone();
        // Check the cache was updated successfully
        assert_eq!(cached_price.result.unwrap().to_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn batching_unsuccessful_estimates() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested just one
            .times(1)
            .returning(|_| {
                Err(PriceEstimationError::NoLiquidity)
            });
        let cache: Arc<TokioMutex<HashMap<H160, CachedResult>>> = Default::default();
        let batching = Batching::new(
            cache.clone(),
            Duration::from_nanos(10),
            Arc::new(native_price_batch_fetcher),
        );

        let result = batching
            .blocking_estimate_prices_and_update_cache(&token(0))
            .await
            .expect("valid result");

        assert_eq!(result, None);
        let cached_price = cache.lock().await.get(&token(0)).cloned();
        // Check the cache was NOT updated successfully
        assert_eq!(cached_price.map(|price| price.result), None);
    }

    // Function to check batching of many tokens
    async fn check_batching_many(
        batching: Arc<Batching>,
        cache: Arc<TokioMutex<HashMap<H160, CachedResult>>>,
        tokens_requested: usize,
    ) {
        let mut futures = Vec::with_capacity(tokens_requested);
        for i in 0..tokens_requested {
            let batching = batching.clone();
            futures.push(tokio::spawn(async move {
                batching
                    .blocking_estimate_prices_and_update_cache(&token(i.try_into().unwrap()))
                    .await
                    .expect("valid result")
            }));
        }

        let mut results = try_join_all(futures).await.expect("valid futures");

        while let Some(result) = results.pop() {
            let result = result.unwrap();
            assert_eq!(result.to_i64().unwrap(), 1);
        }

        let cache = cache.lock().await;
        for i in 0..tokens_requested {
            let cached_price = cache
                .get(&token(i.try_into().unwrap()))
                .cloned()
                .unwrap()
                .result;
            assert_eq!(cached_price.unwrap().to_i64().unwrap(), 1);
        }
    }

    #[tokio::test]
    async fn batching_many_in_one_batch_successful_estimates() {
        let tokens_requested = 20;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly one time because the max batch is 20, so all petitions fit into one batch request
            .times(1)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });
        let cache: Arc<TokioMutex<HashMap<H160, CachedResult>>> = Default::default();
        let batching = Arc::new(Batching::new(
            cache.clone(),
            Duration::from_millis(100),
            Arc::new(native_price_batch_fetcher),
        ));

        check_batching_many(batching, cache, tokens_requested).await;
    }

    #[tokio::test]
    async fn batching_many_in_two_batch_successful_estimates() {
        let tokens_requested = 21;
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We expect this to be requested exactly two times because the max batch is 20, so all petitions fit into one batch request
            .times(2)
            .returning(|input| {
                Ok(input
                    .iter()
                    .map(|token| (*token, 1.0))
                    .collect::<HashMap<_, _>>())
            });
        let cache: Arc<TokioMutex<HashMap<H160, CachedResult>>> = Default::default();
        let batching = Arc::new(Batching::new(
            cache.clone(),
            Duration::from_millis(100),
            Arc::new(native_price_batch_fetcher),
        ));

        check_batching_many(batching, cache, tokens_requested).await;
    }

    #[tokio::test]
    async fn batching_no_calls() {
        let mut native_price_batch_fetcher = MockNativePriceBatchFetcher::new();
        native_price_batch_fetcher
            .expect_max_batch_size()
            .returning(|| 20);
        native_price_batch_fetcher
            .expect_fetch_native_prices()
            // We are testing the native prices are never called
            .never();
        let cache: Arc<TokioMutex<HashMap<H160, CachedResult>>> = Default::default();
        let _batching = Arc::new(Batching::new(
            cache.clone(),
            Duration::from_nanos(50),
            Arc::new(native_price_batch_fetcher),
        ));
        sleep(Duration::from_millis(250)).await;
    }

    #[tokio::test]
    async fn caches_nonrecoverable_failed_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|_| async { Err(PriceEstimationError::NoLiquidity) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            None,
        );

        for _ in 0..10 {
            let result = estimator.estimate_native_price(token(0)).await;
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::NoLiquidity
            ));
        }
    }

    #[tokio::test]
    async fn does_not_cache_recoverable_failed_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(10)
            .returning(|_| async { Err(PriceEstimationError::RateLimited) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            None,
        );

        for _ in 0..10 {
            let result = estimator.estimate_native_price(token(0)).await;
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::RateLimited
            ));
        }
    }

    #[tokio::test]
    async fn maintenance_can_limit_update_size_to_n() {
        let mut inner = MockNativePriceEstimating::new();
        // first request from user
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token| {
                assert_eq!(passed_token, token(0));
                async { Ok(1.0) }.boxed()
            });
        // second request from user
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token| {
                assert_eq!(passed_token, token(1));
                async { Ok(2.0) }.boxed()
            });
        // maintenance task updates n=1 outdated prices
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token| {
                assert_eq!(passed_token, token(1));
                async { Ok(4.0) }.boxed()
            });
        // user requested something which has been skipped by the maintenance task
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token| {
                assert_eq!(passed_token, token(0));
                async { Ok(3.0) }.boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            Some(1),
            Duration::default(),
            1,
            None,
        );

        // fill cache with 2 different queries
        let result = estimator.estimate_native_price(token(0)).await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
        let result = estimator.estimate_native_price(token(1)).await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 2);

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        let result = estimator.estimate_native_price(token(0)).await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 3);

        let result = estimator.estimate_native_price(token(1)).await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 4);
    }

    #[tokio::test]
    async fn maintenance_can_update_all_old_queries() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(10)
            .returning(move |_| async { Ok(1.0) }.boxed());
        // background task updates all outdated prices
        inner
            .expect_estimate_native_price()
            .times(10)
            .returning(move |_| async { Ok(2.0) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            None,
            Duration::default(),
            1,
            None,
        );

        let tokens: Vec<_> = (0..10).map(H160::from_low_u64_be).collect();
        for token in &tokens {
            let price = estimator.estimate_native_price(*token).await.unwrap();
            assert_eq!(price.to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        for token in &tokens {
            let price = estimator.estimate_native_price(*token).await.unwrap();
            assert_eq!(price.to_i64().unwrap(), 2);
        }
    }

    #[tokio::test]
    async fn maintenance_can_update_concurrently() {
        const WAIT_TIME_MS: u64 = 100;
        const BATCH_SIZE: usize = 100;
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(BATCH_SIZE)
            .returning(|_| async { Ok(1.0) }.boxed());
        // background task updates all outdated prices
        inner
            .expect_estimate_native_price()
            .times(BATCH_SIZE)
            .returning(move |_| {
                async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(WAIT_TIME_MS)).await;
                    Ok(2.0)
                }
                .boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            None,
            Duration::default(),
            BATCH_SIZE,
            None,
        );

        let tokens: Vec<_> = (0..BATCH_SIZE as u64).map(H160::from_low_u64_be).collect();
        for token in &tokens {
            let price = estimator.estimate_native_price(*token).await.unwrap();
            assert_eq!(price.to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        // although we have 100 requests which all take 100ms to complete the
        // maintenance cycle completes sooner because all requests are handled
        // concurrently.
        tokio::time::sleep(Duration::from_millis(60 + WAIT_TIME_MS)).await;

        for token in &tokens {
            let price = estimator.estimate_native_price(*token).await.unwrap();
            assert_eq!(price.to_i64().unwrap(), 2);
        }
    }

    #[tokio::test]
    async fn outdated_entries_prioritized() {
        let t0 = H160::from_low_u64_be(0);
        let t1 = H160::from_low_u64_be(1);
        let now = Instant::now();
        let inner = Inner {
            cache: Arc::new(TokioMutex::new(
                [
                    (
                        t0,
                        CachedResult {
                            result: Ok(0.),
                            updated_at: now,
                            requested_at: now,
                        },
                    ),
                    (
                        t1,
                        CachedResult {
                            result: Ok(0.),
                            updated_at: now,
                            requested_at: now,
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            )),
            high_priority: Default::default(),
            estimator: Box::new(MockNativePriceEstimating::new()),
            max_age: Default::default(),
            batching: None,
        };

        let now = now + Duration::from_secs(1);

        *inner.high_priority.lock().unwrap() = std::iter::once(t0).collect();
        let tokens = inner
            .sorted_tokens_to_update(Duration::from_secs(0), now)
            .await;
        assert_eq!(tokens[0], t0);
        assert_eq!(tokens[1], t1);

        *inner.high_priority.lock().unwrap() = std::iter::once(t1).collect();
        let tokens = inner
            .sorted_tokens_to_update(Duration::from_secs(0), now)
            .await;
        assert_eq!(tokens[0], t1);
        assert_eq!(tokens[1], t0);
    }
}
