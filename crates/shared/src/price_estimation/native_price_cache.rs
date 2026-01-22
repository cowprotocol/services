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

// This could be a bool but lets keep it as an enum for clarity.
// Arguably this should not implement Default for the same argument...
/// Determines whether the background maintenance task should
/// keep the token price up to date automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeepPriceUpdated {
    #[default]
    Yes,
    No,
}

// This could be a bool but lets keep it as an enum for clarity.
/// Determines whether we need the price of the token to be
/// actively kept up to date by the maintenance task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiresUpdatingPrices {
    /// The lookup does not care whether the price of the token
    /// is actively being maintained. In other words the flag
    /// of the token should not be changed.
    DontCare,
    /// The token will be marked to require active maintenance.
    Yes,
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
    /// Estimator used for maintenance updates.
    /// Maintenance only refreshes Auction-sourced entries.
    pub estimator: Arc<dyn NativePriceEstimating>,
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

/// Type alias for backwards compatibility.
/// `NativePriceCache` is now `Arc<CacheStorage>` with methods on
/// `CacheStorage`.
pub type NativePriceCache = Arc<CacheStorage>;

/// A cache storage for native price estimates.
///
/// Can be shared between multiple `CachingNativePriceEstimator` instances,
/// allowing them to read/write from the same cache while using different
/// price estimation sources.
pub struct CacheStorage {
    cache: Mutex<HashMap<Address, CachedResult>>,
    max_age: Duration,
    /// Tokens that should be prioritized during maintenance updates.
    high_priority: Mutex<IndexSet<Address>>,
}

impl CacheStorage {
    /// Creates a new cache with the given max age for entries and initial
    /// prices. Entries are initialized with random ages to avoid expiration
    /// spikes.
    fn new(max_age: Duration, initial_prices: HashMap<Address, BigDecimal>) -> Arc<Self> {
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
                        KeepPriceUpdated::Yes,
                    ),
                ))
            })
            .collect::<HashMap<_, _>>();

        Arc::new(Self {
            cache: Mutex::new(cache),
            max_age,
            high_priority: Default::default(),
        })
    }

    /// Creates a new cache with background maintenance task.
    ///
    /// The maintenance task periodically refreshes cached prices before they
    /// expire, using the provided estimator to fetch new prices.
    pub fn new_with_maintenance(
        max_age: Duration,
        initial_prices: HashMap<Address, BigDecimal>,
        config: MaintenanceConfig,
    ) -> Arc<Self> {
        let cache = Self::new(max_age, initial_prices);
        spawn_maintenance_task(&cache, config);
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
    ) -> Arc<Self> {
        Self::new(max_age, initial_prices)
    }

    /// Returns the max age configuration for this cache.
    pub fn max_age(&self) -> Duration {
        self.max_age
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().is_empty()
    }

    /// Get a cached price with optional cache modifications.
    /// Returns None if the price is not cached or is expired.
    ///
    /// The `lookup` parameter controls what modifications to perform:
    /// - `ReadOnly`: No modifications, just check the cache
    /// - `UpgradeOnly`: Upgrade Quote→Auction entries, but don't create missing
    /// - `CreateForMaintenance`: Create missing entries with Auction source and
    ///   upgrade existing Quote→Auction entries
    fn get_cached_price(
        &self,
        token: Address,
        now: Instant,
        require_updating_price: RequiresUpdatingPrices,
    ) -> Option<CachedResult> {
        let mut cache = self.cache.lock().unwrap();
        Self::get_cached_price_inner(&mut cache, token, now, require_updating_price, self.max_age)
    }

    /// Inner implementation of cache lookup that works with an already-locked
    /// cache. This allows both single and batch lookups to share the same
    /// logic.
    fn get_cached_price_inner(
        cache: &mut HashMap<Address, CachedResult>,
        token: Address,
        now: Instant,
        require_updating_price: RequiresUpdatingPrices,
        max_age: Duration,
    ) -> Option<CachedResult> {
        match cache.entry(token) {
            Entry::Occupied(mut entry) => {
                let cached = entry.get_mut();
                cached.requested_at = now;

                if cached.update_price_continuously == KeepPriceUpdated::No
                    && require_updating_price == RequiresUpdatingPrices::Yes
                {
                    tracing::trace!(?token, "marking token for needing active maintenance");
                    cached.update_price_continuously = KeepPriceUpdated::Yes;
                }

                let is_recent = now.saturating_duration_since(cached.updated_at) < max_age;
                is_recent.then_some(cached.clone())
            }
            Entry::Vacant(entry) => {
                if require_updating_price == RequiresUpdatingPrices::Yes {
                    // Create an outdated cache entry so the background task keeping the cache warm
                    // will fetch the price during the next maintenance cycle.
                    // This should happen only for prices missing while building the auction.
                    // Otherwise malicious actors could easily cause the cache size to blow up.
                    let outdated_timestamp = now.checked_sub(max_age).unwrap_or(now);
                    tracing::trace!(?token, "create outdated price entry");
                    entry.insert(CachedResult::new(
                        Ok(0.),
                        outdated_timestamp,
                        now,
                        Default::default(),
                        KeepPriceUpdated::Yes,
                    ));
                }
                None
            }
        }
    }

    /// Get a cached price that is ready to use (not in error accumulation
    /// state).
    ///
    /// Returns None if the price is not cached, is expired, or is not ready to
    /// use.
    fn get_ready_to_use_cached_price(
        &self,
        token: Address,
        now: Instant,
        required_updating_price: RequiresUpdatingPrices,
    ) -> Option<CachedResult> {
        self.get_cached_price(token, now, required_updating_price)
            .filter(|cached| cached.is_ready())
    }

    /// Batch version of `get_ready_to_use_cached_price` that acquires the lock
    /// once for all tokens, improving performance when looking up multiple
    /// prices.
    ///
    /// Returns a HashMap of token addresses to their cached results (only for
    /// tokens that have valid, ready-to-use cached prices).
    fn get_ready_to_use_cached_prices(
        &self,
        tokens: &[Address],
        now: Instant,
        require_updating_price: RequiresUpdatingPrices,
    ) -> HashMap<Address, CachedResult> {
        let mut cache = self.cache.lock().unwrap();
        let mut results = HashMap::with_capacity(tokens.len());

        for token in tokens {
            let cached_result = Self::get_cached_price_inner(
                &mut cache,
                *token,
                now,
                require_updating_price,
                self.max_age,
            );
            if let Some(cached) = cached_result.filter(|c| c.is_ready()) {
                results.insert(*token, cached);
            }
        }

        results
    }

    /// Insert or update a cached result.
    fn insert(&self, token: Address, result: CachedResult) {
        self.cache.lock().unwrap().insert(token, result);
    }

    /// Fetches all tokens that need to be updated sorted by the provided
    /// priority.
    fn prioritized_tokens_to_update(
        &self,
        max_age: Duration,
        now: Instant,
        high_priority: &IndexSet<Address>,
    ) -> Vec<Address> {
        let mut outdated: Vec<_> = self
            .cache
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| {
                cached.update_price_continuously == KeepPriceUpdated::Yes
                    && now.saturating_duration_since(cached.updated_at) > max_age
            })
            .map(|(token, cached)| (*token, cached.requested_at))
            .collect();

        let index = |token: &Address| high_priority.get_index_of(token).unwrap_or(usize::MAX);
        outdated.sort_by_cached_key(|entry| {
            (
                index(&entry.0),            // important items have a low index
                std::cmp::Reverse(entry.1), // important items have recent (i.e. "big") timestamp
            )
        });
        outdated.into_iter().map(|(token, _)| token).collect()
    }

    /// Replaces the set of high-priority tokens with the provided set.
    /// High-priority tokens are refreshed before other tokens in the cache.
    pub fn replace_high_priority(&self, tokens: IndexSet<Address>) {
        tracing::trace!(?tokens, "updated high priority tokens in cache");
        *self.high_priority.lock().unwrap() = tokens;
    }

    // TODO: I think it should be possible to unify this with
    // `estimate_prices_and_update_cache`. Not sure why the new function
    // suddenly has to exist.
    //
    /// Estimates prices for the given tokens and updates the cache.
    /// Used by the background maintenance task. All tokens are processed using
    /// the provided estimator and marked as Auction source.
    fn estimate_prices_and_update_cache_for_maintenance<'a>(
        &'a self,
        tokens: &'a [Address],
        estimator: &'a Arc<dyn NativePriceEstimating>,
        concurrent_requests: usize,
        request_timeout: Duration,
    ) -> futures::stream::BoxStream<'a, (Address, NativePriceEstimateResult)> {
        let estimates = tokens.iter().map(move |token| {
            let estimator = estimator.clone();
            async move {
                let current_accumulative_errors_count = {
                    // check if the price is cached by now
                    let now = Instant::now();

                    match self.get_cached_price(*token, now, RequiresUpdatingPrices::DontCare) {
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

                // update price in cache with Auction source
                if should_cache(&result) {
                    let now = Instant::now();
                    self.insert(
                        *token,
                        CachedResult::new(
                            result.clone(),
                            now,
                            now,
                            current_accumulative_errors_count,
                            KeepPriceUpdated::Yes,
                        ),
                    );
                };

                (*token, result)
            }
        });
        futures::stream::iter(estimates)
            .buffered(concurrent_requests)
            .boxed()
    }
}

/// Spawns a background maintenance task for the given cache.
fn spawn_maintenance_task(cache: &Arc<CacheStorage>, config: MaintenanceConfig) {
    let update_task = CacheMaintenanceTask::new(Arc::downgrade(cache), config)
        .run()
        .instrument(tracing::info_span!("native_price_cache_maintenance"));
    tokio::spawn(update_task);
}

/// Background task that keeps the cache warm by periodically refreshing prices.
/// Only refreshes Auction-sourced entries; Quote-sourced entries are cached
/// but not maintained.
struct CacheMaintenanceTask {
    cache: Weak<CacheStorage>,
    /// Estimator used for maintenance updates.
    estimator: Arc<dyn NativePriceEstimating>,
    update_interval: Duration,
    update_size: Option<usize>,
    prefetch_time: Duration,
    concurrent_requests: usize,
    quote_timeout: Duration,
}

impl CacheMaintenanceTask {
    fn new(cache: Weak<CacheStorage>, config: MaintenanceConfig) -> Self {
        CacheMaintenanceTask {
            cache,
            estimator: config.estimator,
            update_interval: config.update_interval,
            update_size: config.update_size,
            prefetch_time: config.prefetch_time,
            concurrent_requests: config.concurrent_requests,
            quote_timeout: config.quote_timeout,
        }
    }

    /// Single run of the background updating process.
    /// Only updates Auction-sourced entries; Quote-sourced entries are skipped.
    async fn single_update(&self, cache: &Arc<CacheStorage>) {
        let metrics = Metrics::get();
        metrics
            .native_price_cache_size
            .set(i64::try_from(cache.len()).unwrap_or(i64::MAX));

        let max_age = cache.max_age().saturating_sub(self.prefetch_time);
        let high_priority = cache.high_priority.lock().unwrap().clone();
        let mut outdated_entries =
            cache.prioritized_tokens_to_update(max_age, Instant::now(), &high_priority);

        tracing::trace!(tokens = ?outdated_entries, first_n = ?self.update_size, "outdated auction prices to fetch");

        metrics
            .native_price_cache_outdated_entries
            .set(i64::try_from(outdated_entries.len()).unwrap_or(i64::MAX));

        outdated_entries.truncate(self.update_size.unwrap_or(usize::MAX));

        if outdated_entries.is_empty() {
            return;
        }

        let stream = cache.estimate_prices_and_update_cache_for_maintenance(
            &outdated_entries,
            &self.estimator,
            self.concurrent_requests,
            self.quote_timeout,
        );

        let updates_count = stream.count().await as u64;
        metrics
            .native_price_cache_background_updates
            .inc_by(updates_count);
    }

    /// Runs background updates until the cache is no longer alive.
    async fn run(self) {
        while let Some(cache) = self.cache.upgrade() {
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
pub struct CachingNativePriceEstimator {
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
    require_updating_prices: RequiresUpdatingPrices,
}

type CacheEntry = Result<f64, PriceEstimationError>;

#[derive(Debug, Clone)]
struct CachedResult {
    result: CacheEntry,
    updated_at: Instant,
    requested_at: Instant,
    accumulative_errors_count: u32,
    update_price_continuously: KeepPriceUpdated,
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
        update_price_continuously: KeepPriceUpdated,
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
            update_price_continuously,
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

impl CachingNativePriceEstimator {
    /// Returns a reference to the underlying shared cache.
    /// This can be used to share the cache with other estimator instances.
    pub fn cache(&self) -> &Arc<CacheStorage> {
        &self.cache
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
        require_updating_prices: RequiresUpdatingPrices,
    ) -> Self {
        Self {
            estimator,
            cache,
            concurrent_requests,
            approximation_tokens,
            require_updating_prices,
        }
    }

    /// Only returns prices that are currently cached. Missing prices will get
    /// prioritized to get fetched during the next cycles of the maintenance
    /// background task (only for Auction source).
    ///
    /// If this estimator has Auction source and a cached entry has Quote
    /// source, the entry is upgraded to Auction source.
    fn get_cached_prices(
        &self,
        tokens: &[Address],
    ) -> HashMap<Address, Result<f64, PriceEstimationError>> {
        let now = Instant::now();
        let cached_results =
            self.cache
                .get_ready_to_use_cached_prices(tokens, now, self.require_updating_prices);

        let hits = cached_results.len();
        let misses = tokens.len().saturating_sub(hits);
        let metrics = Metrics::get();
        if hits > 0 {
            metrics
                .native_price_cache_access
                .with_label_values(&["hits"])
                .inc_by(hits as u64);
        }
        if misses > 0 {
            metrics
                .native_price_cache_access
                .with_label_values(&["misses"])
                .inc_by(misses as u64);
        }

        cached_results
            .into_iter()
            .map(|(token, cached)| (token, cached.result))
            .collect()
    }

    /// Updates the set of high-priority tokens for maintenance updates.
    /// Forwards to the underlying cache.
    pub fn replace_high_priority(&self, tokens: IndexSet<Address>) {
        self.cache.replace_high_priority(tokens);
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
        let price_stream = self.estimate_prices_and_update_cache(&uncached_tokens, timeout);

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

    /// Checks cache for the given tokens one by one. If the price is already
    /// cached, it gets returned. If it's not in the cache, a new price
    /// estimation request gets issued. We check the cache before each
    /// request because they can take a long time and some other task might
    /// have fetched some requested price in the meantime.
    ///
    /// If this estimator has Auction source and the cached entry has Quote
    /// source, the entry is upgraded to Auction source.
    fn estimate_prices_and_update_cache<'a>(
        &'a self,
        tokens: &'a [Address],
        request_timeout: Duration,
    ) -> futures::stream::BoxStream<'a, (Address, NativePriceEstimateResult)> {
        let estimates = tokens.iter().map(move |token| async move {
            let current_accumulative_errors_count = {
                // check if the price is cached by now
                let now = Instant::now();

                match self
                    .cache
                    .get_cached_price(*token, now, self.require_updating_prices)
                {
                    Some(cached) if cached.is_ready() => {
                        return (*token, cached.result);
                    }
                    Some(cached) => cached.accumulative_errors_count,
                    None => Default::default(),
                }
            };

            let result = self
                .fetch_and_cache_price(*token, request_timeout, current_accumulative_errors_count)
                .await;

            (*token, result)
        });
        futures::stream::iter(estimates)
            .buffered(self.concurrent_requests)
            .boxed()
    }

    /// Fetches a single price and caches it.
    async fn fetch_and_cache_price(
        &self,
        token: Address,
        timeout: Duration,
        accumulative_errors_count: u32,
    ) -> NativePriceEstimateResult {
        let token_to_fetch = *self.approximation_tokens.get(&token).unwrap_or(&token);

        let result = self
            .estimator
            .estimate_native_price(token_to_fetch, timeout)
            .await;

        if should_cache(&result) {
            let now = Instant::now();
            let continuously_update_price = match self.require_updating_prices {
                RequiresUpdatingPrices::Yes => KeepPriceUpdated::Yes,
                RequiresUpdatingPrices::DontCare => KeepPriceUpdated::No,
            };
            self.cache.insert(
                token,
                CachedResult::new(
                    result.clone(),
                    now,
                    now,
                    accumulative_errors_count,
                    continuously_update_price,
                ),
            );
        }

        result
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
            let cached =
                self.cache
                    .get_ready_to_use_cached_price(token, now, self.require_updating_prices);

            let label = if cached.is_some() { "hits" } else { "misses" };
            Metrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);

            if let Some(cached) = cached {
                return cached.result;
            }

            self.estimate_prices_and_update_cache(&[token], timeout)
                .next()
                .await
                .unwrap()
                .1
        }
        .boxed()
    }
}

/// Wrapper around `CachingNativePriceEstimator` that marks all requests as
/// Quote source. Used for the autopilot API endpoints where prices should be
/// cached but not actively maintained by the background task.
#[derive(Clone)]
pub struct QuoteCompetitionEstimator(Arc<CachingNativePriceEstimator>);

impl QuoteCompetitionEstimator {
    /// Creates a new QuoteSourceEstimator wrapping the given estimator.
    ///
    /// Prices fetched through this wrapper will be cached with Quote source,
    /// meaning they won't be actively refreshed by the background maintenance
    /// task. However, if the same token is later requested for auction
    /// purposes, the entry will be upgraded to Auction source and become
    /// actively maintained.
    pub fn new(estimator: Arc<CachingNativePriceEstimator>) -> Self {
        Self(estimator)
    }
}

impl NativePriceEstimating for QuoteCompetitionEstimator {
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> futures::future::BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let now = Instant::now();
            // Quote source doesn't upgrade or create entries, just read
            let cached = self.0.cache.get_ready_to_use_cached_price(
                token,
                now,
                RequiresUpdatingPrices::DontCare,
            );

            let label = if cached.is_some() { "hits" } else { "misses" };
            Metrics::get()
                .native_price_cache_access
                .with_label_values(&[label])
                .inc_by(1);

            if let Some(cached) = cached {
                return cached.result;
            }

            self.0.fetch_and_cache_price(token, timeout, 0).await
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
            CacheStorage::new_without_maintenance(Duration::from_secs(MAX_AGE_SECS), prices);
        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            cache,
            1,
            Default::default(),
            RequiresUpdatingPrices::Yes,
        );

        {
            // Check that `updated_at` timestamps are initialized with
            // reasonable values.
            let cache = estimator.cache.cache.lock().unwrap();
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
            CacheStorage::new_without_maintenance(Duration::from_millis(30), Default::default()),
            1,
            Default::default(),
            RequiresUpdatingPrices::Yes,
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
            CacheStorage::new_without_maintenance(Duration::from_millis(30), Default::default()),
            1,
            // set token approximations for tokens 1 and 2
            HashMap::from([
                (Address::with_last_byte(1), Address::with_last_byte(100)),
                (Address::with_last_byte(2), Address::with_last_byte(200)),
            ]),
            RequiresUpdatingPrices::Yes,
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
            CacheStorage::new_without_maintenance(Duration::from_millis(30), Default::default()),
            1,
            Default::default(),
            RequiresUpdatingPrices::Yes,
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
            CacheStorage::new_without_maintenance(Duration::from_millis(100), Default::default()),
            1,
            Default::default(),
            RequiresUpdatingPrices::Yes,
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
            CacheStorage::new_without_maintenance(Duration::from_millis(30), Default::default()),
            1,
            Default::default(),
            RequiresUpdatingPrices::Yes,
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

        let cache = CacheStorage::new_with_maintenance(
            Duration::from_millis(30),
            Default::default(),
            MaintenanceConfig {
                estimator: Arc::new(maintenance),
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
            RequiresUpdatingPrices::Yes,
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

        let cache = CacheStorage::new_with_maintenance(
            Duration::from_millis(30),
            Default::default(),
            MaintenanceConfig {
                estimator: Arc::new(maintenance),
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
            RequiresUpdatingPrices::Yes,
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

        let cache = CacheStorage::new_with_maintenance(
            Duration::from_millis(30),
            Default::default(),
            MaintenanceConfig {
                estimator: Arc::new(maintenance),
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
            RequiresUpdatingPrices::Yes,
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

        // Create a cache and populate it directly with Auction-sourced entries
        // (since maintenance only updates Auction entries)
        let cache =
            CacheStorage::new_without_maintenance(Duration::from_secs(10), Default::default());
        cache.insert(
            t0,
            CachedResult::new(Ok(0.), now, now, Default::default(), KeepPriceUpdated::Yes),
        );
        cache.insert(
            t1,
            CachedResult::new(Ok(0.), now, now, Default::default(), KeepPriceUpdated::Yes),
        );

        let now = now + Duration::from_secs(1);

        let high_priority: IndexSet<Address> = std::iter::once(t0).collect();
        cache.replace_high_priority(high_priority.clone());
        let tokens =
            cache.prioritized_tokens_to_update(Duration::from_secs(0), now, &high_priority);
        assert_eq!(tokens[0], t0);
        assert_eq!(tokens[1], t1);

        let high_priority: IndexSet<Address> = std::iter::once(t1).collect();
        cache.replace_high_priority(high_priority.clone());
        let tokens =
            cache.prioritized_tokens_to_update(Duration::from_secs(0), now, &high_priority);
        assert_eq!(tokens[0], t1);
        assert_eq!(tokens[1], t0);
    }
}
