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
    prometheus::{IntCounter, IntCounterVec, IntGauge},
    rand::Rng,
    std::{
        collections::{HashMap, HashSet, hash_map::Entry},
        sync::{Arc, Mutex, MutexGuard},
        time::{Duration, Instant},
    },
    tokio::time,
    tracing::{Instrument, instrument},
};

/// Represents a token used for price approximation, including the normalization
/// factor needed to convert between tokens with potentially different decimals.
#[derive(Debug, Clone, Copy)]
pub struct ApproximationToken {
    /// The address of the token to use for price approximation.
    pub address: Address,
    /// The factor to multiply the approximated price by to normalize for
    /// decimal differences. Computed as 10^(to_decimals - from_decimals).
    pub normalization_factor: f64,
}

impl ApproximationToken {
    /// Creates an approximation token with no decimal normalization needed
    /// (both tokens have the same decimals).
    pub fn same_decimals(address: Address) -> Self {
        Self {
            address,
            normalization_factor: 1.0,
        }
    }

    /// Creates an approximation token with the specified normalization factor.
    /// The normalization factor converts prices from the approximation token's
    /// decimal basis to the source token's decimal basis.
    pub fn with_normalization(
        (peg_token, peg_token_decimals): (Address, u8),
        token_decimals: u8,
    ) -> Self {
        let decimals_diff = i32::from(peg_token_decimals) - i32::from(token_decimals);
        Self {
            address: peg_token,
            normalization_factor: 10f64.powi(decimals_diff),
        }
    }

    /// Applies the normalization factor to a price.
    pub fn normalize_price(&self, price: f64) -> f64 {
        price * self.normalization_factor
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct CacheMetrics {
    /// native price cache hits misses
    #[metric(labels("result"))]
    native_price_cache_access: IntCounterVec,
    /// number of items in cache
    native_price_cache_size: IntGauge,
}

impl CacheMetrics {
    fn get() -> &'static Self {
        CacheMetrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct UpdaterMetrics {
    /// number of background updates performed
    native_price_cache_background_updates: IntCounter,
    /// number of items in cache that are outdated
    native_price_cache_outdated_entries: IntGauge,
}

impl UpdaterMetrics {
    fn get() -> &'static Self {
        UpdaterMetrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

type CacheEntry = Result<f64, PriceEstimationError>;

#[derive(Debug, Clone)]
struct CachedResult {
    result: CacheEntry,
    updated_at: Instant,
    requested_at: Instant,
    accumulative_errors_count: u32,
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

/// Passive shared data store for native price cache entries. Clone via internal
/// `Arc`.
#[derive(Clone)]
pub struct Cache(Arc<CacheInner>);

struct CacheInner {
    data: Mutex<HashMap<Address, CachedResult>>,
    max_age: Duration,
}

impl Cache {
    pub fn new(max_age: Duration, initial_prices: HashMap<Address, BigDecimal>) -> Self {
        let mut rng = rand::thread_rng();
        let now = std::time::Instant::now();

        let data = initial_prices
            .into_iter()
            .filter_map(|(token, price)| {
                let updated_at = Self::random_updated_at(max_age, now, &mut rng);
                Some((
                    token,
                    CachedResult::new(
                        Ok(from_normalized_price(price)?),
                        updated_at,
                        now,
                        Default::default(),
                    ),
                ))
            })
            .collect::<HashMap<_, _>>();

        Self(Arc::new(CacheInner {
            data: Mutex::new(data),
            max_age,
        }))
    }

    pub fn max_age(&self) -> Duration {
        self.0.max_age
    }

    /// Returns a randomized `updated_at` timestamp that is 50-90% of max_age
    /// in the past, to avoid spikes of expired prices all being fetched at
    /// once.
    fn random_updated_at(max_age: Duration, now: Instant, rng: &mut impl Rng) -> Instant {
        let percent_expired = rng.gen_range(50..=90);
        let age = max_age.as_secs() * percent_expired / 100;
        now - Duration::from_secs(age)
    }

    pub fn len(&self) -> usize {
        self.0.data.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.data.lock().unwrap().is_empty()
    }

    fn get_cached_price(
        token: Address,
        now: Instant,
        cache: &mut MutexGuard<HashMap<Address, CachedResult>>,
        max_age: &Duration,
        create_missing_entry: bool,
    ) -> Option<CachedResult> {
        match cache.entry(token) {
            Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.requested_at = now;
                let is_recent = now.saturating_duration_since(entry.updated_at) < *max_age;
                is_recent.then_some(entry.clone())
            }
            Entry::Vacant(entry) => {
                if create_missing_entry {
                    // Create an outdated placeholder entry so it can be picked
                    // up by the next price update. This should happen only for
                    // prices missing while building the auction. Otherwise
                    // malicious actors could easily cause the cache size to blow
                    // up.
                    let outdated_timestamp = now.checked_sub(*max_age).unwrap();
                    tracing::trace!(?token, "create outdated price entry");
                    entry.insert(CachedResult::new(
                        Ok(0.),
                        outdated_timestamp,
                        now,
                        Default::default(),
                    ));
                }
                None
            }
        }
    }

    fn get_ready_to_use_cached_price(
        token: Address,
        now: Instant,
        cache: &mut MutexGuard<HashMap<Address, CachedResult>>,
        max_age: &Duration,
        create_missing_entry: bool,
    ) -> Option<CachedResult> {
        Self::get_cached_price(token, now, cache, max_age, create_missing_entry)
            .filter(|cached| cached.is_ready())
    }

    /// Only returns prices that are currently cached. When
    /// `create_missing_entries` is true, missing tokens get outdated
    /// placeholder entries so they can be picked up by the next price
    /// update.
    pub fn get_cached_prices(
        &self,
        tokens: &[Address],
        create_missing_entries: bool,
    ) -> HashMap<Address, Result<f64, PriceEstimationError>> {
        let now = Instant::now();
        let mut cache = self.0.data.lock().unwrap();
        let mut results = HashMap::default();
        for token in tokens {
            let cached = Self::get_ready_to_use_cached_price(
                *token,
                now,
                &mut cache,
                &self.0.max_age,
                create_missing_entries,
            );
            let label = if cached.is_some() { "hits" } else { "misses" };
            CacheMetrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);
            if let Some(result) = cached {
                results.insert(*token, result.result);
            }
        }
        results
    }

    fn insert(&self, token: Address, result: CachedResult) {
        self.0.data.lock().unwrap().insert(token, result);
    }

    /// Returns tokens whose cached price is outdated.
    fn outdated_tokens(&self, max_age: Duration, now: Instant) -> Vec<Address> {
        self.0
            .data
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > max_age)
            .map(|(token, _)| *token)
            .collect()
    }
}

/// Wrapper around `Box<dyn NativePriceEstimating>` which caches successful
/// price estimates for some time. Does not spawn any background tasks.
///
/// Is an Arc internally.
#[derive(Clone)]
pub struct CachingNativePriceEstimator(Arc<CachingInner>);

struct CachingInner {
    estimator: Box<dyn NativePriceEstimating>,
    cache: Cache,
    concurrent_requests: usize,
    // TODO remove when implementing a less hacky solution
    /// Maps a requested token to an approximating token. If the system
    /// wants to get the native price for the requested token the native
    /// price of the approximating token should be fetched and returned instead.
    /// This can be useful for tokens that are hard to route but are pegged to
    /// the same underlying asset so approximating their native prices is deemed
    /// safe (e.g. csUSDL => Dai).
    /// The normalization factor handles decimal differences between tokens.
    /// After startup this is a read only value.
    approximation_tokens: HashMap<Address, ApproximationToken>,
    quote_timeout: Duration,
}

impl CachingInner {
    /// Checks cache for the given tokens one by one. If the price is already
    /// cached, it gets returned. If it's not in the cache, a new price
    /// estimation request gets issued. We check the cache before each
    /// request because they can take a long time and some other task might
    /// have fetched some requested price in the meantime.
    fn estimate_prices_and_update_cache<'a>(
        &'a self,
        tokens: &'a [Address],
        max_age: Duration,
        request_timeout: Duration,
    ) -> futures::stream::BoxStream<'a, (Address, NativePriceEstimateResult)> {
        let estimates = tokens.iter().map(move |token| async move {
            let current_accumulative_errors_count = {
                // check if the price is cached by now
                let now = Instant::now();
                let mut cache = self.cache.0.data.lock().unwrap();

                match Cache::get_cached_price(*token, now, &mut cache, &max_age, false) {
                    Some(cached) if cached.is_ready() => {
                        return (*token, cached.result);
                    }
                    Some(cached) => cached.accumulative_errors_count,
                    None => Default::default(),
                }
            };

            let approximation = self
                .approximation_tokens
                .get(token)
                .copied()
                .unwrap_or(ApproximationToken::same_decimals(*token));

            let result = self
                .estimator
                .estimate_native_price(approximation.address, request_timeout)
                .await
                .map(|price| approximation.normalize_price(price));

            // update price in cache
            if should_cache(&result) {
                let now = Instant::now();
                self.cache.insert(
                    *token,
                    CachedResult::new(result.clone(), now, now, current_accumulative_errors_count),
                );
            };

            (*token, result)
        });
        futures::stream::iter(estimates)
            .buffered(self.concurrent_requests)
            .boxed()
    }
}

impl CachingNativePriceEstimator {
    pub fn new(
        estimator: Box<dyn NativePriceEstimating>,
        cache: Cache,
        concurrent_requests: usize,
        approximation_tokens: HashMap<Address, ApproximationToken>,
        quote_timeout: Duration,
    ) -> Self {
        let inner = Arc::new(CachingInner {
            estimator,
            cache,
            concurrent_requests,
            approximation_tokens,
            quote_timeout,
        });
        Self(inner)
    }

    pub fn cache(&self) -> &Cache {
        &self.0.cache
    }

    /// Only returns prices that are currently cached. Missing tokens get
    /// outdated placeholder entries so they can be picked up by the next price
    /// update.
    pub fn get_cached_prices(
        &self,
        tokens: &[Address],
    ) -> HashMap<Address, Result<f64, PriceEstimationError>> {
        self.0.cache.get_cached_prices(tokens, true)
    }

    pub async fn fetch_prices(
        &self,
        tokens: &[Address],
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
        let price_stream = self.0.estimate_prices_and_update_cache(
            &uncached_tokens,
            self.0.cache.max_age(),
            timeout,
        );

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
            let cached = {
                let now = Instant::now();
                let mut cache = self.0.cache.0.data.lock().unwrap();
                Cache::get_ready_to_use_cached_price(
                    token,
                    now,
                    &mut cache,
                    &self.0.cache.0.max_age,
                    false,
                )
            };

            let label = if cached.is_some() { "hits" } else { "misses" };
            CacheMetrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);

            if let Some(cached) = cached {
                return cached.result;
            }

            self.0
                .estimate_prices_and_update_cache(&[token], self.0.cache.max_age(), timeout)
                .next()
                .await
                .unwrap()
                .1
        }
        .boxed()
    }
}

/// Background maintenance worker that periodically updates native prices
/// for a set of tokens. Uses a `CachingNativePriceEstimator` for fetching
/// and caching prices.
pub struct NativePriceUpdater {
    estimator: CachingNativePriceEstimator,
    tokens_to_update: Mutex<HashSet<Address>>,
}

impl NativePriceUpdater {
    pub fn new(
        estimator: CachingNativePriceEstimator,
        update_interval: Duration,
        prefetch_time: Duration,
    ) -> Arc<Self> {
        assert!(
            estimator.cache().max_age() > prefetch_time,
            "price cache prefetch time ({:?}) must be less than max age ({:?})",
            prefetch_time,
            estimator.cache().max_age(),
        );

        let updater = Arc::new(Self {
            estimator,
            tokens_to_update: Default::default(),
        });

        // Don't keep the updater alive just for the background task
        let weak = Arc::downgrade(&updater);
        let update_task = async move {
            while let Some(updater) = weak.upgrade() {
                let now = Instant::now();
                updater.single_update(prefetch_time).await;
                drop(updater);
                tokio::time::sleep(update_interval.saturating_sub(now.elapsed())).await;
            }
        }
        .instrument(tracing::info_span!("native_price_updater"));
        tokio::spawn(update_task);

        updater
    }

    pub fn cache(&self) -> &Cache {
        self.estimator.cache()
    }

    /// Replaces the full set of tokens that should be maintained by the
    /// background task.
    pub fn set_tokens_to_update(&self, tokens: HashSet<Address>) {
        tracing::trace!(?tokens, "update tokens to maintain");
        *self.tokens_to_update.lock().unwrap() = tokens;
    }

    pub async fn fetch_prices(
        &self,
        tokens: &[Address],
        timeout: Duration,
    ) -> HashMap<Address, NativePriceEstimateResult> {
        self.estimator.fetch_prices(tokens, timeout).await
    }

    async fn single_update(&self, prefetch_time: Duration) {
        let metrics = UpdaterMetrics::get();
        let cache = self.estimator.cache();

        CacheMetrics::get()
            .native_price_cache_size
            .set(i64::try_from(cache.len()).unwrap_or(i64::MAX));

        let max_age = cache.max_age().saturating_sub(prefetch_time);
        let tokens_to_update = self.tokens_to_update.lock().unwrap().clone();

        // Ensure all tokens_to_update have entries in the cache so they get
        // maintained.
        {
            let now = Instant::now();
            let mut rng = rand::thread_rng();
            let mut data = cache.0.data.lock().unwrap();
            for token in &tokens_to_update {
                if let Entry::Vacant(entry) = data.entry(*token) {
                    let updated_at = Cache::random_updated_at(cache.0.max_age, now, &mut rng);
                    entry.insert(CachedResult::new(
                        Ok(0.),
                        updated_at,
                        now,
                        Default::default(),
                    ));
                }
            }
        }

        let outdated_entries = cache.outdated_tokens(max_age, Instant::now());

        tracing::trace!(count = outdated_entries.len(), "outdated prices to fetch");

        metrics
            .native_price_cache_outdated_entries
            .set(i64::try_from(outdated_entries.len()).unwrap_or(i64::MAX));

        if outdated_entries.is_empty() {
            return;
        }

        let timeout = self.estimator.0.quote_timeout;
        let mut stream =
            self.estimator
                .0
                .estimate_prices_and_update_cache(&outdated_entries, max_age, timeout);
        while stream.next().await.is_some() {}
        metrics
            .native_price_cache_background_updates
            .inc_by(outdated_entries.len() as u64);
    }
}

impl NativePriceEstimating for NativePriceUpdater {
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        self.estimator.estimate_native_price(token, timeout)
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

    /// Helper to create a CachingNativePriceEstimator with its own Cache
    /// (convenience for tests that don't need a separate Cache).
    fn create_caching_estimator(
        inner: MockNativePriceEstimating,
        max_age: Duration,
        concurrent_requests: usize,
        approximation_tokens: HashMap<Address, ApproximationToken>,
    ) -> CachingNativePriceEstimator {
        let cache = Cache::new(max_age, Default::default());
        CachingNativePriceEstimator::new(
            Box::new(inner),
            cache,
            concurrent_requests,
            approximation_tokens,
            HEALTHY_PRICE_ESTIMATION_TIME,
        )
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
        let cache = Cache::new(Duration::from_secs(MAX_AGE_SECS), prices);
        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            cache,
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );

        {
            // Check that `updated_at` timestamps are initialized with
            // reasonable values.
            let data = estimator.cache().0.data.lock().unwrap();
            for value in data.values() {
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

        let estimator =
            create_caching_estimator(inner, Duration::from_millis(30), 1, Default::default());

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

        let estimator = create_caching_estimator(
            inner,
            Duration::from_millis(30),
            1,
            // set token approximations for tokens 1 and 2 (same decimals)
            HashMap::from([
                (
                    Address::with_last_byte(1),
                    ApproximationToken::same_decimals(Address::with_last_byte(100)),
                ),
                (
                    Address::with_last_byte(2),
                    ApproximationToken::same_decimals(Address::with_last_byte(200)),
                ),
            ]),
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
    async fn approximation_normalizes_when_target_has_more_decimals() {
        // Scenario: Token 1 is USDC-like (6 decimals), approximated by DAI-like token
        // 100 (18 decimals) Both worth $1, so they're pegged 1:1 in value
        let mut inner = MockNativePriceEstimating::new();
        // DAI-like token returns price of 5e-22 ETH per wei (smallest unit)
        inner
            .expect_estimate_native_price()
            .times(1)
            .withf(move |t, _| *t == token(100))
            .returning(|_, _| async { Ok(5e-22) }.boxed());

        // from_decimals=6 (USDC), to_decimals=18 (DAI)
        // Normalization factor = 10^(18-6) = 10^12
        // Price should be 5e-22 * 10^12 = 5e-10 ETH per USDC microunit
        let estimator = create_caching_estimator(
            inner,
            Duration::from_millis(30),
            1,
            HashMap::from([(
                Address::with_last_byte(1),
                ApproximationToken::with_normalization((Address::with_last_byte(100), 18), 6),
            )]),
        );

        let price = estimator
            .estimate_native_price(Address::with_last_byte(1), HEALTHY_PRICE_ESTIMATION_TIME)
            .await
            .unwrap();
        // 5e-22 * 10^12 = 5e-10
        // Note: small floating point error due to 10^12 not being exactly representable
        let expected = 5e-10;
        assert!(
            (price - expected).abs() / expected < f64::EPSILON,
            "price {price} not within relative epsilon of {expected}"
        );
    }

    #[tokio::test]
    async fn approximation_normalizes_when_target_has_fewer_decimals() {
        // Scenario: Token 1 is DAI-like (18 decimals), approximated by USDC-like token
        // 100 (6 decimals) Both worth $1, so they're pegged 1:1 in value
        let mut inner = MockNativePriceEstimating::new();
        // USDC-like token returns price of 5e-10 ETH per microunit (smallest unit)
        inner
            .expect_estimate_native_price()
            .times(1)
            .withf(move |t, _| *t == token(100))
            .returning(|_, _| async { Ok(5e-10) }.boxed());

        // from_decimals=18 (DAI), to_decimals=6 (USDC)
        // Normalization factor = 10^(6-18) = 10^-12
        // Price should be 5e-10 * 10^-12 = 5e-22 ETH per DAI wei
        let estimator = create_caching_estimator(
            inner,
            Duration::from_millis(30),
            1,
            HashMap::from([(
                Address::with_last_byte(1),
                ApproximationToken::with_normalization((Address::with_last_byte(100), 6), 18),
            )]),
        );

        let price = estimator
            .estimate_native_price(Address::with_last_byte(1), HEALTHY_PRICE_ESTIMATION_TIME)
            .await
            .unwrap();
        // 5e-10 * 10^-12 = 5e-22
        // Note: small floating point error due to 10^-12 not being exactly
        // representable
        let expected = 5e-22;
        assert!(
            (price - expected).abs() / expected < f64::EPSILON,
            "price {price} not within relative epsilon of {expected}"
        );
    }

    #[tokio::test]
    async fn caches_nonrecoverable_failed_estimates() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| async { Err(PriceEstimationError::NoLiquidity) }.boxed());

        let estimator =
            create_caching_estimator(inner, Duration::from_millis(30), 1, Default::default());

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

        let estimator =
            create_caching_estimator(inner, Duration::from_millis(100), 1, Default::default());

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

        let estimator =
            create_caching_estimator(inner, Duration::from_millis(30), 1, Default::default());

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
        let mut inner = MockNativePriceEstimating::new();
        // first request from user
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token, _| {
                assert_eq!(passed_token, token(0));
                async { Ok(1.0) }.boxed()
            });
        // second request from user
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token, _| {
                assert_eq!(passed_token, token(1));
                async { Ok(2.0) }.boxed()
            });
        // maintenance task updates outdated prices (order is non-deterministic)
        inner
            .expect_estimate_native_price()
            .times(2)
            .returning(|passed_token, _| {
                let price = if passed_token == token(0) { 3.0 } else { 4.0 };
                async move { Ok(price) }.boxed()
            });

        let cache = Cache::new(Duration::from_millis(30), Default::default());
        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            cache,
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let updater = NativePriceUpdater::new(
            estimator.clone(),
            Duration::from_millis(50),
            Duration::default(),
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

        // Tell the updater about these tokens
        updater.set_tokens_to_update([token(0), token(1)].into_iter().collect());

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60)).await;

        let result = estimator
            .estimate_native_price(token(0), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 3);

        let result = estimator
            .estimate_native_price(token(1), HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.as_ref().unwrap().to_i64().unwrap(), 4);
    }

    #[tokio::test]
    async fn maintenance_can_update_all_old_queries() {
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(10)
            .returning(move |_, _| async { Ok(1.0) }.boxed());
        inner
            .expect_estimate_native_price()
            .times(10)
            .returning(move |_, _| async { Ok(2.0) }.boxed());

        let cache = Cache::new(Duration::from_millis(30), Default::default());
        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            cache,
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let all_tokens: HashSet<_> = (0..10).map(Address::with_last_byte).collect();
        let updater = NativePriceUpdater::new(
            estimator.clone(),
            Duration::from_millis(50),
            Duration::default(),
        );
        updater.set_tokens_to_update(all_tokens);

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
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(BATCH_SIZE)
            .returning(|_, _| async { Ok(1.0) }.boxed());
        inner
            .expect_estimate_native_price()
            .times(BATCH_SIZE)
            .returning(move |_, _| {
                async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(WAIT_TIME_MS)).await;
                    Ok(2.0)
                }
                .boxed()
            });

        let cache = Cache::new(Duration::from_millis(30), Default::default());
        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            cache,
            BATCH_SIZE,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let all_tokens: HashSet<_> = (0..BATCH_SIZE as u64)
            .map(|u| Address::left_padding_from(&u.to_be_bytes()))
            .collect();
        let updater = NativePriceUpdater::new(
            estimator.clone(),
            Duration::from_millis(50),
            Duration::default(),
        );
        updater.set_tokens_to_update(all_tokens);

        let tokens: Vec<_> = (0..BATCH_SIZE as u64).map(token).collect();
        for token in &tokens {
            let price = estimator
                .estimate_native_price(*token, HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap();
            assert_eq!(price.to_i64().unwrap(), 1);
        }

        // wait for maintenance cycle
        tokio::time::sleep(Duration::from_millis(60 + WAIT_TIME_MS)).await;

        for token in &tokens {
            let price = estimator
                .estimate_native_price(*token, HEALTHY_PRICE_ESTIMATION_TIME)
                .await
                .unwrap();
            assert_eq!(price.to_i64().unwrap(), 2);
        }
    }
}
