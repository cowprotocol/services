use {
    super::PriceEstimationError,
    crate::price_estimation::native::{
        NativePriceEstimateResult,
        NativePriceEstimating,
        from_normalized_price,
    },
    alloy::primitives::Address,
    bigdecimal::BigDecimal,
    futures::{FutureExt, StreamExt},
    indexmap::IndexSet,
    prometheus::{IntCounter, IntCounterVec, IntGauge},
    rand::Rng,
    std::{
        collections::{HashMap, hash_map::Entry},
        sync::{Arc, Mutex, Weak},
        time::{Duration, Instant},
    },
    tokio::time,
    tracing::{Instrument, instrument},
};

/// Identifies which estimator type fetched a cached entry.
/// Used by maintenance to dispatch to the correct estimator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EstimatorSource {
    /// Primary estimator - the main source for price estimates.
    #[default]
    Primary,
    /// Secondary estimator - supplementary source that may have different
    /// price sources.
    Secondary,
}

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
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

/// Configuration for the background maintenance task that keeps the cache warm.
pub struct MaintenanceConfig {
    /// Map of estimators by source type for maintenance.
    /// Maintenance dispatches to the appropriate estimator based on
    /// which source originally fetched each cached entry.
    pub estimators: HashMap<EstimatorSource, Arc<dyn NativePriceEstimating>>,
    /// How often to run the maintenance task.
    pub update_interval: Duration,
    /// Maximum number of prices to update per maintenance cycle.
    /// None means unlimited.
    pub update_size: Option<usize>,
    /// How early before expiration to refresh prices.
    pub prefetch_time: Duration,
    /// Number of concurrent price fetch requests.
    pub concurrent_requests: usize,
    /// Timeout for individual price fetch requests.
    pub quote_timeout: Duration,
}

/// A cache storage for native price estimates.
///
/// Can be shared between multiple `CachingNativePriceEstimator` instances,
/// allowing them to read/write from the same cache while using different
/// price estimation sources.
#[derive(Clone)]
pub struct NativePriceCache {
    inner: Arc<CacheStorage>,
}

struct CacheStorage {
    cache: Mutex<HashMap<Address, CachedResult>>,
    max_age: Duration,
    /// Tokens that should be prioritized during maintenance updates.
    high_priority: Mutex<IndexSet<Address>>,
}

impl NativePriceCache {
    /// Creates a new cache with the given max age for entries and initial
    /// prices. Entries are initialized with random ages to avoid expiration
    /// spikes.
    fn new(max_age: Duration, initial_prices: HashMap<Address, BigDecimal>) -> Self {
        let mut rng = rand::thread_rng();
        let now = std::time::Instant::now();

        let cache = initial_prices
            .into_iter()
            .filter_map(|(token, price)| {
                // Generate random `updated_at` timestamp
                // to avoid spikes of expired prices.
                let percent_expired = rng.gen_range(50..=90);
                let age = max_age.as_secs() * percent_expired / 100;
                let updated_at = now - Duration::from_secs(age);

                Some((
                    token,
                    CachedResult::new(
                        Ok(from_normalized_price(price)?),
                        updated_at,
                        now,
                        Default::default(),
                        EstimatorSource::default(),
                    ),
                ))
            })
            .collect::<HashMap<_, _>>();

        Self {
            inner: Arc::new(CacheStorage {
                cache: Mutex::new(cache),
                max_age,
                high_priority: Default::default(),
            }),
        }
    }

    /// Creates a new cache with background maintenance task.
    ///
    /// The maintenance task periodically refreshes cached prices before they
    /// expire, using the provided estimator to fetch new prices.
    pub fn new_with_maintenance(
        max_age: Duration,
        initial_prices: HashMap<Address, BigDecimal>,
        config: MaintenanceConfig,
    ) -> Self {
        let cache = Self::new(max_age, initial_prices);
        cache.spawn_maintenance_task(config);
        cache
    }

    /// Creates a new cache without background maintenance.
    ///
    /// This is only available for testing purposes. Production code should use
    /// `new_with_maintenance` instead.
    #[cfg(any(test, feature = "test-util"))]
    pub fn new_without_maintenance(
        max_age: Duration,
        initial_prices: HashMap<Address, BigDecimal>,
    ) -> Self {
        Self::new(max_age, initial_prices)
    }

    /// Returns the max age configuration for this cache.
    pub fn max_age(&self) -> Duration {
        self.inner.max_age
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.inner.cache.lock().unwrap().len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a cached price, optionally creating a placeholder entry for missing
    /// tokens. Returns None if the price is not cached or is expired.
    /// If `create_missing_entry` is Some, creates an outdated entry with the
    /// given source type so maintenance will fetch it.
    fn get_cached_price(
        &self,
        token: Address,
        now: Instant,
        create_missing_entry: Option<EstimatorSource>,
    ) -> Option<CachedResult> {
        let mut cache = self.inner.cache.lock().unwrap();
        match cache.entry(token) {
            Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.requested_at = now;
                let is_recent =
                    now.saturating_duration_since(entry.updated_at) < self.inner.max_age;
                is_recent.then_some(entry.clone())
            }
            Entry::Vacant(entry) => {
                if let Some(source) = create_missing_entry {
                    // Create an outdated cache entry so the background task keeping the cache warm
                    // will fetch the price during the next maintenance cycle.
                    // This should happen only for prices missing while building the auction.
                    // Otherwise malicious actors could easily cause the cache size to blow up.
                    let outdated_timestamp = now.checked_sub(self.inner.max_age).unwrap();
                    tracing::trace!(?token, "create outdated price entry");
                    entry.insert(CachedResult::new(
                        Ok(0.),
                        outdated_timestamp,
                        now,
                        Default::default(),
                        source,
                    ));
                }
                None
            }
        }
    }

    /// Get a cached price that is ready to use (not in error accumulation
    /// state).
    fn get_ready_to_use_cached_price(
        &self,
        token: Address,
        now: Instant,
        create_missing_entry: Option<EstimatorSource>,
    ) -> Option<CachedResult> {
        self.get_cached_price(token, now, create_missing_entry)
            .filter(|cached| cached.is_ready())
    }

    /// Insert or update a cached result.
    fn insert(&self, token: Address, result: CachedResult) {
        self.inner.cache.lock().unwrap().insert(token, result);
    }

    /// Get tokens that need updating with their sources, sorted by priority.
    fn sorted_tokens_to_update_with_sources(
        &self,
        max_age: Duration,
        now: Instant,
        high_priority: &IndexSet<Address>,
    ) -> Vec<(Address, EstimatorSource)> {
        let mut outdated: Vec<_> = self
            .inner
            .cache
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > max_age)
            .map(|(token, cached)| (*token, cached.requested_at, cached.source))
            .collect();

        let index = |token: &Address| high_priority.get_index_of(token).unwrap_or(usize::MAX);
        outdated.sort_by_cached_key(|entry| {
            (
                index(&entry.0),            // important items have a low index
                std::cmp::Reverse(entry.1), // important items have recent (i.e. "big") timestamp
            )
        });
        outdated
            .into_iter()
            .map(|(token, _, source)| (token, source))
            .collect()
    }

    /// Updates the set of high-priority tokens for maintenance updates.
    /// High-priority tokens are refreshed before other tokens in the cache.
    pub fn replace_high_priority(&self, tokens: IndexSet<Address>) {
        tracing::trace!(?tokens, "update high priority tokens in cache");
        *self.inner.high_priority.lock().unwrap() = tokens;
    }

    /// Spawns a background maintenance task for this cache.
    fn spawn_maintenance_task(&self, config: MaintenanceConfig) {
        let update_task = CacheUpdateTask {
            cache: Arc::downgrade(&self.inner),
            estimators: config.estimators,
            update_interval: config.update_interval,
            update_size: config.update_size,
            prefetch_time: config.prefetch_time,
            concurrent_requests: config.concurrent_requests,
            quote_timeout: config.quote_timeout,
        }
        .run()
        .instrument(tracing::info_span!("native_price_cache_maintenance"));
        tokio::spawn(update_task);
    }

    /// Estimates prices for the given tokens and updates the cache.
    /// Used by the background maintenance task. Each token is processed using
    /// the estimator corresponding to its source.
    fn estimate_prices_and_update_cache<'a>(
        &'a self,
        tokens: &'a [(Address, EstimatorSource)],
        estimators: &'a HashMap<EstimatorSource, Arc<dyn NativePriceEstimating>>,
        concurrent_requests: usize,
        request_timeout: Duration,
    ) -> futures::stream::BoxStream<'a, (Address, NativePriceEstimateResult)> {
        let estimates = tokens.iter().filter_map(move |(token, source)| {
            let source = *source;
            let estimator = estimators.get(&source)?.clone();
            Some(async move {
                let current_accumulative_errors_count = {
                    // check if the price is cached by now
                    let now = Instant::now();

                    match self.get_cached_price(*token, now, None) {
                        Some(cached) if cached.is_ready() => {
                            return (*token, cached.result);
                        }
                        Some(cached) => cached.accumulative_errors_count,
                        None => Default::default(),
                    }
                };

                let result = estimator
                    .estimate_native_price(*token, request_timeout)
                    .await;

                // update price in cache
                if should_cache(&result) {
                    let now = Instant::now();
                    self.insert(
                        *token,
                        CachedResult::new(
                            result.clone(),
                            now,
                            now,
                            current_accumulative_errors_count,
                            source,
                        ),
                    );
                };

                (*token, result)
            })
        });
        futures::stream::iter(estimates)
            .buffered(concurrent_requests)
            .boxed()
    }
}

/// Background task that keeps the cache warm by periodically refreshing prices.
struct CacheUpdateTask {
    cache: Weak<CacheStorage>,
    /// Map of estimators by source type. Maintenance dispatches to the
    /// appropriate estimator based on which source fetched each entry.
    estimators: HashMap<EstimatorSource, Arc<dyn NativePriceEstimating>>,
    update_interval: Duration,
    update_size: Option<usize>,
    prefetch_time: Duration,
    concurrent_requests: usize,
    quote_timeout: Duration,
}

impl CacheUpdateTask {
    /// Single run of the background updating process.
    async fn single_update(&self, cache: &NativePriceCache) {
        let metrics = Metrics::get();
        metrics
            .native_price_cache_size
            .set(i64::try_from(cache.len()).unwrap_or(i64::MAX));

        let max_age = cache.max_age().saturating_sub(self.prefetch_time);
        let high_priority = cache.inner.high_priority.lock().unwrap().clone();
        let mut outdated_entries =
            cache.sorted_tokens_to_update_with_sources(max_age, Instant::now(), &high_priority);

        tracing::trace!(tokens = ?outdated_entries, first_n = ?self.update_size, "outdated prices to fetch");

        metrics
            .native_price_cache_outdated_entries
            .set(i64::try_from(outdated_entries.len()).unwrap_or(i64::MAX));

        outdated_entries.truncate(self.update_size.unwrap_or(usize::MAX));

        if outdated_entries.is_empty() {
            return;
        }

        let mut stream = cache.estimate_prices_and_update_cache(
            &outdated_entries,
            &self.estimators,
            self.concurrent_requests,
            self.quote_timeout,
        );

        let mut updates_count = 0u64;
        while stream.next().await.is_some() {
            updates_count += 1;
        }

        metrics
            .native_price_cache_background_updates
            .inc_by(updates_count);
    }

    /// Runs background updates until the cache is no longer alive.
    async fn run(self) {
        while let Some(inner) = self.cache.upgrade() {
            let cache = NativePriceCache { inner };
            let now = Instant::now();
            self.single_update(&cache).await;
            tokio::time::sleep(self.update_interval.saturating_sub(now.elapsed())).await;
        }
    }
}

/// Wrapper around `Arc<dyn NativePriceEstimating>` which caches successful
/// price estimates for some time and supports updating the cache in the
/// background.
///
/// The size of the underlying cache is unbounded.
///
/// Is an Arc internally.
#[derive(Clone)]
pub struct CachingNativePriceEstimator(Arc<Inner>);

struct Inner {
    cache: NativePriceCache,
    estimator: Arc<dyn NativePriceEstimating>,
    concurrent_requests: usize,
    // TODO remove when implementing a less hacky solution
    /// Maps a requested token to an approximating token. If the system
    /// wants to get the native price for the requested token the native
    /// price of the approximating token should be fetched and returned instead.
    /// This can be useful for tokens that are hard to route but are pegged to
    /// the same underlying asset so approximating their native prices is deemed
    /// safe (e.g. csUSDL => Dai).
    /// It's very important that the 2 tokens have the same number of decimals.
    /// After startup this is a read only value.
    approximation_tokens: HashMap<Address, Address>,
    /// Identifies which estimator type this is, used to track which
    /// estimator fetched each cached entry for proper maintenance.
    source: EstimatorSource,
}

type CacheEntry = Result<f64, PriceEstimationError>;

#[derive(Debug, Clone)]
struct CachedResult {
    result: CacheEntry,
    updated_at: Instant,
    requested_at: Instant,
    accumulative_errors_count: u32,
    /// Which estimator type fetched this entry.
    source: EstimatorSource,
}

/// Defines how many consecutive errors are allowed before the cache starts
/// returning the error to the user without trying to fetch the price from the
/// estimator.
const ACCUMULATIVE_ERRORS_THRESHOLD: u32 = 5;

impl CachedResult {
    fn new(
        result: CacheEntry,
        updated_at: Instant,
        requested_at: Instant,
        current_accumulative_errors_count: u32,
        source: EstimatorSource,
    ) -> Self {
        let estimator_internal_errors_count =
            matches!(result, Err(PriceEstimationError::EstimatorInternal(_)))
                .then_some(current_accumulative_errors_count + 1)
                .unwrap_or_default();

        Self {
            result,
            updated_at,
            requested_at,
            accumulative_errors_count: estimator_internal_errors_count,
            source,
        }
    }

    /// The result is not ready if the estimator has returned an internal error
    /// and consecutive errors are less than
    /// `ESTIMATOR_INTERNAL_ERRORS_THRESHOLD`.
    fn is_ready(&self) -> bool {
        !matches!(self.result, Err(PriceEstimationError::EstimatorInternal(_)))
            || self.accumulative_errors_count >= ACCUMULATIVE_ERRORS_THRESHOLD
    }
}

impl Inner {
    /// Checks cache for the given tokens one by one. If the price is already
    /// cached, it gets returned. If it's not in the cache, a new price
    /// estimation request gets issued. We check the cache before each
    /// request because they can take a long time and some other task might
    /// have fetched some requested price in the meantime.
    fn estimate_prices_and_update_cache<'a>(
        &'a self,
        tokens: &'a [Address],
        request_timeout: Duration,
    ) -> futures::stream::BoxStream<'a, (Address, NativePriceEstimateResult)> {
        let estimates = tokens.iter().map(move |token| async move {
            let current_accumulative_errors_count = {
                // check if the price is cached by now
                let now = Instant::now();

                match self.cache.get_cached_price(*token, now, None) {
                    Some(cached) if cached.is_ready() => {
                        return (*token, cached.result);
                    }
                    Some(cached) => cached.accumulative_errors_count,
                    None => Default::default(),
                }
            };

            let token_to_fetch = *self.approximation_tokens.get(token).unwrap_or(token);

            let result = self
                .estimator
                .estimate_native_price(token_to_fetch, request_timeout)
                .await;

            // update price in cache
            if should_cache(&result) {
                let now = Instant::now();
                self.cache.insert(
                    *token,
                    CachedResult::new(
                        result.clone(),
                        now,
                        now,
                        current_accumulative_errors_count,
                        self.source,
                    ),
                );
            };

            (*token, result)
        });
        futures::stream::iter(estimates)
            .buffered(self.concurrent_requests)
            .boxed()
    }
}

fn should_cache(result: &Result<f64, PriceEstimationError>) -> bool {
    // We don't want to cache errors that we consider transient
    match result {
        Ok(_)
        | Err(PriceEstimationError::NoLiquidity)
        | Err(PriceEstimationError::UnsupportedToken { .. })
        | Err(PriceEstimationError::EstimatorInternal(_)) => true,
        Err(PriceEstimationError::ProtocolInternal(_)) | Err(PriceEstimationError::RateLimited) => {
            false
        }
        Err(PriceEstimationError::UnsupportedOrderType(_)) => {
            tracing::error!(?result, "Unexpected error in native price cache");
            false
        }
    }
}

impl CachingNativePriceEstimator {
    /// Returns a reference to the underlying shared cache.
    /// This can be used to share the cache with other estimator instances.
    pub fn cache(&self) -> &NativePriceCache {
        &self.0.cache
    }

    /// Creates a new CachingNativePriceEstimator.
    ///
    /// The estimator will use the provided cache for lookups and will fetch
    /// prices on-demand for cache misses. Background maintenance (keeping the
    /// cache warm) is handled by the cache itself, not by this estimator.
    ///
    /// The `source` parameter identifies which estimator type this is, so that
    /// the maintenance task knows which estimator to use when refreshing
    /// entries fetched by this estimator.
    pub fn new(
        estimator: Arc<dyn NativePriceEstimating>,
        cache: NativePriceCache,
        concurrent_requests: usize,
        approximation_tokens: HashMap<Address, Address>,
        source: EstimatorSource,
    ) -> Self {
        Self(Arc::new(Inner {
            estimator,
            cache,
            concurrent_requests,
            approximation_tokens,
            source,
        }))
    }

    /// Only returns prices that are currently cached. Missing prices will get
    /// prioritized to get fetched during the next cycles of the maintenance
    /// background task.
    fn get_cached_prices(
        &self,
        tokens: &[Address],
    ) -> HashMap<Address, Result<f64, PriceEstimationError>> {
        let now = Instant::now();
        let mut results = HashMap::default();
        for token in tokens {
            // Pass our source so that if a missing entry is created, it's tagged
            // with our source for proper maintenance later.
            let cached =
                self.0
                    .cache
                    .get_ready_to_use_cached_price(*token, now, Some(self.0.source));
            let label = if cached.is_some() { "hits" } else { "misses" };
            Metrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);
            if let Some(result) = cached {
                results.insert(*token, result.result);
            }
        }
        results
    }

    /// Updates the set of high-priority tokens for maintenance updates.
    /// Forwards to the underlying cache.
    pub fn replace_high_priority(&self, tokens: IndexSet<Address>) {
        self.0.cache.replace_high_priority(tokens);
    }

    pub async fn estimate_native_prices_with_timeout<'a>(
        &'a self,
        tokens: &'a [Address],
        timeout: Duration,
    ) -> HashMap<Address, NativePriceEstimateResult> {
        let mut prices = self.get_cached_prices(tokens);
        if timeout.is_zero() {
            return prices;
        }

        let uncached_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| !prices.contains_key(*t))
            .copied()
            .collect();
        let price_stream = self
            .0
            .estimate_prices_and_update_cache(&uncached_tokens, timeout);

        let _ = time::timeout(timeout, async {
            let mut price_stream = price_stream;

            while let Some((token, result)) = price_stream.next().await {
                prices.insert(token, result);
            }
        })
        .await;

        // Return whatever was collected up to that point, regardless of the timeout
        prices
    }
}

impl NativePriceEstimating for CachingNativePriceEstimator {
    #[instrument(skip_all)]
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let now = Instant::now();
            let cached = self.0.cache.get_ready_to_use_cached_price(token, now, None);

            let label = if cached.is_some() { "hits" } else { "misses" };
            Metrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);

            if let Some(cached) = cached {
                return cached.result;
            }

            self.0
                .estimate_prices_and_update_cache(&[token], timeout)
                .next()
                .await
                .unwrap()
                .1
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::{
            HEALTHY_PRICE_ESTIMATION_TIME,
            PriceEstimationError,
            native::{MockNativePriceEstimating, NativePriceEstimating},
        },
        anyhow::anyhow,
        futures::FutureExt,
        num::ToPrimitive,
    };

    fn token(u: u64) -> Address {
        Address::left_padding_from(&u.to_be_bytes())
    }

    #[tokio::test]
    async fn caches_successful_estimates_with_loaded_prices() {
        let mut inner = MockNativePriceEstimating::new();
        inner.expect_estimate_native_price().never();

        const MAX_AGE_SECS: u64 = 600;
        let min_age = Duration::from_secs(MAX_AGE_SECS * 49 / 100);
        let max_age = Duration::from_secs(MAX_AGE_SECS * 91 / 100);

        let prices =
            HashMap::from_iter((0..10).map(|t| (token(t), BigDecimal::try_from(1e18).unwrap())));
        let cache =
            NativePriceCache::new_without_maintenance(Duration::from_secs(MAX_AGE_SECS), prices);
        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            cache,
            1,
            Default::default(),
            Default::default(),
        );

        {
            // Check that `updated_at` timestamps are initialized with
            // reasonable values.
            let cache = estimator.0.cache.inner.cache.lock().unwrap();
            for value in cache.values() {
                let elapsed = value.updated_at.elapsed();
                assert!(elapsed >= min_age && elapsed <= max_age);
            }
        }

        for i in 0..10 {
            let result = estimator
                .estimate_native_price(token(i), HEALTHY_PRICE_ESTIMATION_TIME)
                .await;
            assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
        }
    }

    #[tokio::test]
    async fn caches_successful_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| async { Ok(1.0) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            NativePriceCache::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
            ),
            1,
            Default::default(),
            Default::default(),
        );

        for _ in 0..10 {
            let result = estimator
                .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
                .await;
            assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
        }
    }

    #[tokio::test]
    async fn caches_approximated_estimates_use() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(1)
            .withf(move |t, _| *t == token(0))
            .returning(|_, _| async { Ok(1.0) }.boxed());
        inner
            .expect_estimate_native_price()
            .times(1)
            .withf(move |t, _| *t == token(100))
            .returning(|_, _| async { Ok(100.0) }.boxed());
        inner
            .expect_estimate_native_price()
            .times(1)
            .withf(move |t, _| *t == token(200))
            .returning(|_, _| async { Ok(200.0) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            NativePriceCache::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
            ),
            1,
            // set token approximations for tokens 1 and 2
            HashMap::from([
                (Address::with_last_byte(1), Address::with_last_byte(100)),
                (Address::with_last_byte(2), Address::with_last_byte(200)),
            ]),
            Default::default(),
        );

        // no approximation token used for token 0
        assert_eq!(
            estimator
                .estimate_native_price(Address::with_last_byte(0), HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap()
                .to_i64()
                .unwrap(),
            1
        );

        // approximation price used for tokens 1 and 2
        assert_eq!(
            estimator
                .estimate_native_price(Address::with_last_byte(1), HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap()
                .to_i64()
                .unwrap(),
            100
        );
        assert_eq!(
            estimator
                .estimate_native_price(Address::with_last_byte(2), HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap()
                .to_i64()
                .unwrap(),
            200
        );
    }

    #[tokio::test]
    async fn caches_nonrecoverable_failed_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| async { Err(PriceEstimationError::NoLiquidity) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            NativePriceCache::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
            ),
            1,
            Default::default(),
            Default::default(),
        );

        for _ in 0..10 {
            let result = estimator
                .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
                .await;
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::NoLiquidity
            ));
        }
    }

    #[tokio::test]
    async fn properly_caches_accumulative_errors() {
        let mut inner = MockNativePriceEstimating::new();
        let mut seq = mockall::Sequence::new();

        // First 3 calls: Return EstimatorInternal error. Increment the errors counter.
        inner
            .expect_estimate_native_price()
            .times(3)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                async { Err(PriceEstimationError::EstimatorInternal(anyhow!("boom"))) }.boxed()
            });

        // Next 1 call: Return Ok(1.0). This resets the errors counter.
        inner
            .expect_estimate_native_price()
            .once()
            .in_sequence(&mut seq)
            .returning(|_, _| async { Ok(1.0) }.boxed());

        // Next 2 calls: Return EstimatorInternal error. Start incrementing the errors
        // counter from the beginning.
        inner
            .expect_estimate_native_price()
            .times(2)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                async { Err(PriceEstimationError::EstimatorInternal(anyhow!("boom"))) }.boxed()
            });

        // Next call: Return a recoverable error, which doesn't affect the errors
        // counter.
        inner
            .expect_estimate_native_price()
            .once()
            .in_sequence(&mut seq)
            .returning(|_, _| async { Err(PriceEstimationError::RateLimited) }.boxed());

        // Since the ACCUMULATIVE_ERRORS_THRESHOLD is 5, there are only 3 more calls
        // remain. Anything exceeding that must return the cached value.
        inner
            .expect_estimate_native_price()
            .times(3)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                async { Err(PriceEstimationError::EstimatorInternal(anyhow!("boom"))) }.boxed()
            });

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            NativePriceCache::new_without_maintenance(
                Duration::from_millis(100),
                Default::default(),
            ),
            1,
            Default::default(),
            Default::default(),
        );

        // First 3 calls: The cache is not used. Counter gets increased.
        for _ in 0..3 {
            let result = estimator
                .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
                .await;
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::EstimatorInternal(_)
            ));
        }

        // Reset the errors counter.
        let result = estimator
            .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);

        // Make sure the cached value gets evicted.
        tokio::time::sleep(Duration::from_millis(120)).await;

        // Increment the errors counter again.
        for _ in 0..2 {
            let result = estimator
                .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
                .await;
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::EstimatorInternal(_)
            ));
        }

        // Receive a recoverable error, which shouldn't affect the counter.
        let result = estimator
            .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert!(matches!(
            result.as_ref().unwrap_err(),
            PriceEstimationError::RateLimited
        ));

        // Make more than expected calls. The cache should be used once the threshold is
        // reached.
        for _ in 0..(ACCUMULATIVE_ERRORS_THRESHOLD * 2) {
            let result = estimator
                .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
                .await;
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::EstimatorInternal(_)
            ));
        }
    }

    #[tokio::test]
    async fn does_not_cache_recoverable_failed_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(10)
            .returning(|_, _| async { Err(PriceEstimationError::RateLimited) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            NativePriceCache::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
            ),
            1,
            Default::default(),
            Default::default(),
        );

        for _ in 0..10 {
            let result = estimator
                .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
                .await;
            assert!(matches!(
                result.as_ref().unwrap_err(),
                PriceEstimationError::RateLimited
            ));
        }
    }

    #[tokio::test]
    async fn maintenance_can_limit_update_size_to_n() {
        // On-demand estimator for initial cache population
        let mut on_demand = MockNativePriceEstimating::new();
        on_demand
            .expect_estimate_native_price()
            .times(2)
            .returning(|passed_token, _| {
                let price = if passed_token == token(0) { 1.0 } else { 2.0 };
                async move { Ok(price) }.boxed()
            });
        // After maintenance skips token(0), user request triggers on-demand fetch
        on_demand
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token, _| {
                assert_eq!(passed_token, token(0));
                async { Ok(3.0) }.boxed()
            });

        // Maintenance estimator updates n=1 outdated prices (most recently requested)
        let mut maintenance = MockNativePriceEstimating::new();
        maintenance
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token, _| {
                assert_eq!(passed_token, token(1));
                async { Ok(4.0) }.boxed()
            });

        let cache = NativePriceCache::new_with_maintenance(
            Duration::from_millis(30),
            Default::default(),
            MaintenanceConfig {
                estimators: HashMap::from([(EstimatorSource::Primary, Arc::new(maintenance) as _)]),
                update_interval: Duration::from_millis(50),
                update_size: Some(1),
                prefetch_time: Default::default(),
                concurrent_requests: 1,
                quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
            },
        );

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(on_demand),
            cache,
            1,
            Default::default(),
            Default::default(),
        );

        // fill cache with 2 different queries
        let result = estimator
            .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 1);
        let result = estimator
            .estimate_native_price(token(1), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 2);

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        // token(0) was not updated by maintenance (n=1 limit), triggers on-demand
        let result = estimator
            .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 3);

        // token(1) was updated by maintenance
        let result = estimator
            .estimate_native_price(token(1), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 4);
    }

    #[tokio::test]
    async fn maintenance_can_update_all_old_queries() {
        // On-demand estimator for initial cache population
        let mut on_demand = MockNativePriceEstimating::new();
        on_demand
            .expect_estimate_native_price()
            .times(10)
            .returning(move |_, _| async { Ok(1.0) }.boxed());

        // Maintenance estimator updates all outdated prices
        let mut maintenance = MockNativePriceEstimating::new();
        maintenance
            .expect_estimate_native_price()
            .times(10)
            .returning(move |_, _| async { Ok(2.0) }.boxed());

        let cache = NativePriceCache::new_with_maintenance(
            Duration::from_millis(30),
            Default::default(),
            MaintenanceConfig {
                estimators: HashMap::from([(EstimatorSource::Primary, Arc::new(maintenance) as _)]),
                update_interval: Duration::from_millis(50),
                update_size: None,
                prefetch_time: Default::default(),
                concurrent_requests: 1,
                quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
            },
        );

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(on_demand),
            cache,
            1,
            Default::default(),
            Default::default(),
        );

        let tokens: Vec<_> = (0..10).map(Address::with_last_byte).collect();
        for token in &tokens {
            let price = estimator
                .estimate_native_price(*token, HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap();
            assert_eq!(price.to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        for token in &tokens {
            let price = estimator
                .estimate_native_price(*token, HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap();
            assert_eq!(price.to_i64().unwrap(), 2);
        }
    }

    #[tokio::test]
    async fn maintenance_can_update_concurrently() {
        const WAIT_TIME_MS: u64 = 100;
        const BATCH_SIZE: usize = 100;

        // On-demand estimator for initial cache population
        let mut on_demand = MockNativePriceEstimating::new();
        on_demand
            .expect_estimate_native_price()
            .times(BATCH_SIZE)
            .returning(|_, _| async { Ok(1.0) }.boxed());

        // Maintenance estimator updates all outdated prices (with delay to test
        // concurrency)
        let mut maintenance = MockNativePriceEstimating::new();
        maintenance
            .expect_estimate_native_price()
            .times(BATCH_SIZE)
            .returning(move |_, _| {
                async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(WAIT_TIME_MS)).await;
                    Ok(2.0)
                }
                .boxed()
            });

        let cache = NativePriceCache::new_with_maintenance(
            Duration::from_millis(30),
            Default::default(),
            MaintenanceConfig {
                estimators: HashMap::from([(EstimatorSource::Primary, Arc::new(maintenance) as _)]),
                update_interval: Duration::from_millis(50),
                update_size: None,
                prefetch_time: Default::default(),
                concurrent_requests: BATCH_SIZE,
                quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
            },
        );

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(on_demand),
            cache,
            1,
            Default::default(),
            Default::default(),
        );

        let tokens: Vec<_> = (0..BATCH_SIZE as u64).map(token).collect();
        for token in &tokens {
            let price = estimator
                .estimate_native_price(*token, HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap();
            assert_eq!(price.to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        // although we have 100 requests which all take 100ms to complete the
        // maintenance cycle completes sooner because all requests are handled
        // concurrently.
        tokio::time::sleep(Duration::from_millis(60 + WAIT_TIME_MS)).await;

        for token in &tokens {
            let price = estimator
                .estimate_native_price(*token, HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap();
            assert_eq!(price.to_i64().unwrap(), 2);
        }
    }

    #[test]
    fn outdated_entries_prioritized() {
        let t0 = Address::with_last_byte(0);
        let t1 = Address::with_last_byte(1);
        let now = Instant::now();

        // Create a cache and populate it directly
        let cache =
            NativePriceCache::new_without_maintenance(Duration::from_secs(10), Default::default());
        cache.insert(
            t0,
            CachedResult::new(Ok(0.), now, now, Default::default(), Default::default()),
        );
        cache.insert(
            t1,
            CachedResult::new(Ok(0.), now, now, Default::default(), Default::default()),
        );

        let now = now + Duration::from_secs(1);

        let high_priority: IndexSet<Address> = std::iter::once(t0).collect();
        cache.replace_high_priority(high_priority.clone());
        let tokens =
            cache.sorted_tokens_to_update_with_sources(Duration::from_secs(0), now, &high_priority);
        assert_eq!(tokens[0].0, t0);
        assert_eq!(tokens[1].0, t1);

        let high_priority: IndexSet<Address> = std::iter::once(t1).collect();
        cache.replace_high_priority(high_priority.clone());
        let tokens =
            cache.sorted_tokens_to_update_with_sources(Duration::from_secs(0), now, &high_priority);
        assert_eq!(tokens[0].0, t1);
        assert_eq!(tokens[1].0, t0);
    }
}
