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
        sync::{Arc, Mutex, MutexGuard, Weak},
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

/// Wrapper around `Box<dyn PriceEstimating>` which caches successful price
/// estimates for some time and supports updating the cache in the background.
///
/// The size of the underlying cache is unbounded.
///
/// Is an Arc internally.
#[derive(Clone)]
pub struct CachingNativePriceEstimator(Arc<Inner>);

struct Inner {
    cache: Mutex<HashMap<Address, CachedResult>>,
    high_priority: Mutex<IndexSet<Address>>,
    estimator: Box<dyn NativePriceEstimating>,
    max_age: Duration,
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

struct UpdateTask {
    inner: Weak<Inner>,
    update_interval: Duration,
    update_size: Option<usize>,
    prefetch_time: Duration,
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

impl Inner {
    // Returns a single cached price and updates its `requested_at` field.
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
                    // Create an outdated cache entry so the background task keeping the cache warm
                    // will fetch the price during the next maintenance cycle.
                    // This should happen only for prices missing while building the auction.
                    // Otherwise malicious actors could easily cause the cache size to blow up.
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
                let mut cache = self.cache.lock().unwrap();

                match Self::get_cached_price(*token, now, &mut cache, &max_age, false) {
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
                let mut cache = self.cache.lock().unwrap();

                cache.insert(
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

    /// Tokens with highest priority first.
    fn sorted_tokens_to_update(&self, max_age: Duration, now: Instant) -> Vec<Address> {
        let mut outdated: Vec<_> = self
            .cache
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > max_age)
            .map(|(token, cached)| (*token, cached.requested_at))
            .collect();

        let high_priority = self.high_priority.lock().unwrap().clone();
        let index = |token: &Address| high_priority.get_index_of(token).unwrap_or(usize::MAX);
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

impl UpdateTask {
    /// Single run of the background updating process.
    async fn single_update(&self, inner: &Inner) {
        let metrics = Metrics::get();
        metrics
            .native_price_cache_size
            .set(i64::try_from(inner.cache.lock().unwrap().len()).unwrap_or(i64::MAX));

        let max_age = inner.max_age.saturating_sub(self.prefetch_time);
        let mut outdated_entries = inner.sorted_tokens_to_update(max_age, Instant::now());

        tracing::trace!(tokens = ?outdated_entries, first_n = ?self.update_size, "outdated prices to fetch");

        metrics
            .native_price_cache_outdated_entries
            .set(i64::try_from(outdated_entries.len()).unwrap_or(i64::MAX));

        outdated_entries.truncate(self.update_size.unwrap_or(usize::MAX));

        if outdated_entries.is_empty() {
            return;
        }

        let mut stream =
            inner.estimate_prices_and_update_cache(&outdated_entries, max_age, inner.quote_timeout);
        while stream.next().await.is_some() {}
        metrics
            .native_price_cache_background_updates
            .inc_by(outdated_entries.len() as u64);
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
    pub fn initialize_cache(&self, prices: HashMap<Address, BigDecimal>) {
        let mut rng = rand::thread_rng();
        let now = std::time::Instant::now();

        let cache = prices
            .into_iter()
            .filter_map(|(token, price)| {
                // Generate random `updated_at` timestamp
                // to avoid spikes of expired prices.
                let percent_expired = rng.gen_range(50..=90);
                let age = self.0.max_age.as_secs() * percent_expired / 100;
                let updated_at = now - Duration::from_secs(age);

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

        *self.0.cache.lock().unwrap() = cache;
    }

    /// Creates new CachingNativePriceEstimator using `estimator` to calculate
    /// native prices which get cached a duration of `max_age`.
    /// Spawns a background task maintaining the cache once per
    /// `update_interval`. Only soon to be outdated prices get updated and
    /// recently used prices have a higher priority. If `update_size` is
    /// `Some(n)` at most `n` prices get updated per interval.
    /// If `update_size` is `None` no limit gets applied.
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        estimator: Box<dyn NativePriceEstimating>,
        max_age: Duration,
        update_interval: Duration,
        update_size: Option<usize>,
        prefetch_time: Duration,
        concurrent_requests: usize,
        approximation_tokens: HashMap<Address, ApproximationToken>,
        quote_timeout: Duration,
    ) -> Self {
        let inner = Arc::new(Inner {
            estimator,
            cache: Default::default(),
            high_priority: Default::default(),
            max_age,
            concurrent_requests,
            approximation_tokens,
            quote_timeout,
        });

        let update_task = UpdateTask {
            inner: Arc::downgrade(&inner),
            update_interval,
            update_size,
            prefetch_time,
        }
        .run()
        .instrument(tracing::info_span!("caching_native_price_estimator"));
        tokio::spawn(update_task);

        Self(inner)
    }

    /// Only returns prices that are currently cached. Missing prices will get
    /// prioritized to get fetched during the next cycles of the maintenance
    /// background task.
    fn get_cached_prices(
        &self,
        tokens: &[Address],
    ) -> HashMap<Address, Result<f64, PriceEstimationError>> {
        let now = Instant::now();
        let mut cache = self.0.cache.lock().unwrap();
        let mut results = HashMap::default();
        for token in tokens {
            let cached = Inner::get_ready_to_use_cached_price(
                *token,
                now,
                &mut cache,
                &self.0.max_age,
                true,
            );
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

    pub fn replace_high_priority(&self, tokens: IndexSet<Address>) {
        tracing::trace!(?tokens, "update high priority tokens");
        *self.0.high_priority.lock().unwrap() = tokens;
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
        let price_stream =
            self.0
                .estimate_prices_and_update_cache(&uncached_tokens, self.0.max_age, timeout);

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
                let mut cache = self.0.cache.lock().unwrap();
                Inner::get_ready_to_use_cached_price(token, now, &mut cache, &self.0.max_age, false)
            };

            let label = if cached.is_some() { "hits" } else { "misses" };
            Metrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);

            if let Some(cached) = cached {
                return cached.result;
            }

            self.0
                .estimate_prices_and_update_cache(&[token], self.0.max_age, timeout)
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
        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_secs(MAX_AGE_SECS),
            Default::default(),
            None,
            Default::default(),
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        estimator.initialize_cache(prices);

        {
            // Check that `updated_at` timestamps are initialized with
            // reasonable values.
            let cache = estimator.0.cache.lock().unwrap();
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
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
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
            HEALTHY_PRICE_ESTIMATION_TIME,
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
        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            HashMap::from([(
                Address::with_last_byte(1),
                ApproximationToken::with_normalization((Address::with_last_byte(100), 18), 6),
            )]),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            HashMap::from([(
                Address::with_last_byte(1),
                ApproximationToken::with_normalization((Address::with_last_byte(100), 6), 18),
            )]),
            HEALTHY_PRICE_ESTIMATION_TIME,
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

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
            Box::new(inner),
            Duration::from_millis(100),
            Duration::from_millis(200),
            None,
            Default::default(),
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
            Box::new(inner),
            Duration::from_millis(30),
            Default::default(),
            None,
            Default::default(),
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
        // maintenance task updates n=1 outdated prices
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token, _| {
                assert_eq!(passed_token, token(1));
                async { Ok(4.0) }.boxed()
            });
        // user requested something which has been skipped by the maintenance task
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|passed_token, _| {
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
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
        // background task updates all outdated prices
        inner
            .expect_estimate_native_price()
            .times(10)
            .returning(move |_, _| async { Ok(2.0) }.boxed());

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            None,
            Duration::default(),
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
        let mut inner = MockNativePriceEstimating::new();
        inner
            .expect_estimate_native_price()
            .times(BATCH_SIZE)
            .returning(|_, _| async { Ok(1.0) }.boxed());
        // background task updates all outdated prices
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

        let estimator = CachingNativePriceEstimator::new(
            Box::new(inner),
            Duration::from_millis(30),
            Duration::from_millis(50),
            None,
            Duration::default(),
            BATCH_SIZE,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
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
        let inner = Inner {
            cache: Mutex::new(
                [
                    (t0, CachedResult::new(Ok(0.), now, now, Default::default())),
                    (t1, CachedResult::new(Ok(0.), now, now, Default::default())),
                ]
                .into_iter()
                .collect(),
            ),
            high_priority: Default::default(),
            estimator: Box::new(MockNativePriceEstimating::new()),
            max_age: Default::default(),
            concurrent_requests: 1,
            approximation_tokens: Default::default(),
            quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        let now = now + Duration::from_secs(1);

        *inner.high_priority.lock().unwrap() = std::iter::once(t0).collect();
        let tokens = inner.sorted_tokens_to_update(Duration::from_secs(0), now);
        assert_eq!(tokens[0], t0);
        assert_eq!(tokens[1], t1);

        *inner.high_priority.lock().unwrap() = std::iter::once(t1).collect();
        let tokens = inner.sorted_tokens_to_update(Duration::from_secs(0), now);
        assert_eq!(tokens[0], t1);
        assert_eq!(tokens[1], t0);
    }
}
