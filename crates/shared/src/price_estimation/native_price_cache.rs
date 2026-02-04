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

/// Determines whether the background maintenance task should
/// keep the token price up to date automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeepPriceUpdated {
    #[default]
    Yes,
    No,
}

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
    /// native price cache hits misses by result and caller type
    /// Labels: result=hits|misses, caller=auction|quote
    #[metric(labels("result", "caller"))]
    native_price_cache_access: IntCounterVec,
    /// number of items in cache
    native_price_cache_size: IntGauge,
    /// number of background updates performed
    native_price_cache_background_updates: IntCounter,
    /// number of items in cache that are outdated
    native_price_cache_outdated_entries: IntGauge,
    /// number of entries actively maintained by background task
    /// (KeepPriceUpdated::Yes)
    native_price_cache_maintained_entries: IntGauge,
    /// number of entries passively cached but not maintained
    /// (KeepPriceUpdated::No, i.e. quote-only tokens)
    native_price_cache_passive_entries: IntGauge,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    /// Resets counters on startup to ensure clean metrics for this run.
    fn reset(&self) {
        for caller in &["auction", "quote"] {
            self.native_price_cache_access
                .with_label_values(&["hits", caller])
                .reset();
            self.native_price_cache_access
                .with_label_values(&["misses", caller])
                .reset();
        }
        self.native_price_cache_background_updates.reset();
    }
}

/// Configuration for the background maintenance task that keeps the cache warm.
pub struct MaintenanceConfig {
    /// Estimator used for maintenance updates.
    /// Maintenance only refreshes entries marked with `KeepPriceUpdated::Yes`.
    pub estimator: Arc<dyn NativePriceEstimating>,
    /// How often to run the maintenance task.
    pub update_interval: Duration,
    /// Maximum number of prices to update per maintenance cycle.
    /// 0 means unlimited. High-priority tokens are updated first, so if this
    /// limit is smaller than the number of outdated high-priority tokens,
    /// non-priority tokens won't be updated until the backlog clears.
    pub update_size: usize,
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
    ///
    /// These tokens are updated before non-priority tokens during each
    /// maintenance cycle. Note: If the number of outdated high-priority tokens
    /// exceeds `MaintenanceConfig::update_size`, only that many will be updated
    /// per cycle (in priority order), and non-priority tokens won't be updated
    /// until the high-priority backlog clears.
    high_priority: Mutex<IndexSet<Address>>,
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
}

impl CacheStorage {
    /// Creates a new cache with the given max age for entries and initial
    /// prices. Entries are initialized with random ages to avoid expiration
    /// spikes.
    fn new(
        max_age: Duration,
        initial_prices: HashMap<Address, BigDecimal>,
        approximation_tokens: HashMap<Address, Address>,
    ) -> Arc<Self> {
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
            approximation_tokens,
        })
    }

    /// Creates a new cache with background maintenance task.
    ///
    /// The maintenance task periodically refreshes cached prices before they
    /// expire, using the provided estimator to fetch new prices.
    pub fn new_with_maintenance(
        max_age: Duration,
        initial_prices: HashMap<Address, BigDecimal>,
        approximation_tokens: HashMap<Address, Address>,
        config: MaintenanceConfig,
    ) -> Arc<Self> {
        let cache = Self::new(max_age, initial_prices, approximation_tokens);
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
        approximation_tokens: HashMap<Address, Address>,
    ) -> Arc<Self> {
        Self::new(max_age, initial_prices, approximation_tokens)
    }

    /// Returns the max age configuration for this cache.
    pub fn max_age(&self) -> Duration {
        self.max_age
    }

    /// Returns the approximation tokens mapping.
    pub fn approximation_tokens(&self) -> &HashMap<Address, Address> {
        &self.approximation_tokens
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().is_empty()
    }

    /// Returns counts of (maintained, passive) entries.
    /// Maintained entries have `KeepPriceUpdated::Yes` and are actively
    /// refreshed by the background task. Passive entries have
    /// `KeepPriceUpdated::No` and are only cached (quote-only tokens).
    fn count_by_maintenance_flag(&self) -> (usize, usize) {
        let cache = self.cache.lock().unwrap();
        let maintained = cache
            .values()
            .filter(|c| c.update_price_continuously == KeepPriceUpdated::Yes)
            .count();
        let passive = cache.len() - maintained;
        (maintained, passive)
    }

    /// Get a cached price with optional cache modifications.
    /// Returns None if the price is not cached or is expired.
    ///
    /// The `require_updating_price` parameter controls whether to mark the
    /// token for active price maintenance:
    /// - `DontCare`: Don't modify the token's maintenance flag
    /// - `Yes`: Mark the token to require active price updates
    fn lookup_cached_price(
        cache: &mut HashMap<Address, CachedResult>,
        token: Address,
        now: Instant,
        max_age: Duration,
        require_updating_price: RequiresUpdatingPrices,
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
                    // Create an outdated cache entry so the background task keeping the
                    // cache warm will fetch the price during the next maintenance cycle.
                    // This should happen only for prices missing while building the auction.
                    // Otherwise malicious actors could easily cause the cache size to blow
                    // up.
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

    /// Get cached prices that are ready to use (not in error accumulation
    /// state).
    ///
    /// Returns a map of token -> cached result for tokens that have valid
    /// cached prices. Missing tokens (not cached or expired) are not
    /// included in the result. Also updates cache access metrics
    /// (hits/misses).
    ///
    /// The `require_updating_price` parameter controls whether to mark tokens
    /// for active price maintenance:
    /// - `DontCare`: Don't modify the token's maintenance flag
    /// - `Yes`: Mark the token to require active price updates. For existing
    ///   entries, upgrades `KeepPriceUpdated::No` to `Yes`. For missing tokens,
    ///   creates placeholder entries so the maintenance task will fetch them.
    fn get_ready_to_use_cached_prices(
        &self,
        tokens: &[Address],
        now: Instant,
        require_updating_price: RequiresUpdatingPrices,
    ) -> HashMap<Address, CachedResult> {
        let max_age = self.max_age;
        let outdated_timestamp = now.checked_sub(max_age).unwrap_or(now);
        let mut results = HashMap::new();
        let mut hits = 0u64;
        let mut misses = 0u64;

        let mut cache = self.cache.lock().unwrap();
        for &token in tokens {
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
                    if is_recent && cached.is_ready() {
                        results.insert(token, cached.clone());
                        hits += 1;
                    } else {
                        misses += 1;
                    }
                }
                Entry::Vacant(entry) => {
                    if require_updating_price == RequiresUpdatingPrices::Yes {
                        // Create an outdated cache entry so the background task keeping the
                        // cache warm will fetch the price during the next maintenance cycle.
                        tracing::trace!(?token, "create outdated price entry for maintenance");
                        entry.insert(CachedResult::new(
                            Ok(0.),
                            outdated_timestamp,
                            now,
                            Default::default(),
                            KeepPriceUpdated::Yes,
                        ));
                    }
                    misses += 1;
                }
            }
        }

        drop(cache); // Release lock before metrics update

        let caller = match require_updating_price {
            RequiresUpdatingPrices::Yes => "auction",
            RequiresUpdatingPrices::DontCare => "quote",
        };
        let metrics = Metrics::get();
        if hits > 0 {
            metrics
                .native_price_cache_access
                .with_label_values(&["hits", caller])
                .inc_by(hits);
        }
        if misses > 0 {
            metrics
                .native_price_cache_access
                .with_label_values(&["misses", caller])
                .inc_by(misses);
        }

        results
    }

    /// Insert or update a cached result.
    ///
    /// Note: This locks the cache. Do not call in a loop; prefer batch
    /// operations instead.
    fn insert(&self, token: Address, result: CachedResult) {
        self.cache.lock().unwrap().insert(token, result);
    }

    /// Insert or update multiple cached results in a single lock acquisition.
    fn insert_batch(&self, results: impl IntoIterator<Item = (Address, CachedResult)>) {
        let mut cache = self.cache.lock().unwrap();
        for (token, result) in results {
            cache.insert(token, result);
        }
    }

    /// Get accumulative error counts for multiple tokens in a single lock.
    /// Returns a map of token -> error count. Tokens not in cache return 0.
    fn get_accumulative_errors(&self, tokens: &[Address]) -> HashMap<Address, u32> {
        let cache = self.cache.lock().unwrap();
        tokens
            .iter()
            .map(|&token| {
                let count = cache
                    .get(&token)
                    .map(|c| c.accumulative_errors_count)
                    .unwrap_or_default();
                (token, count)
            })
            .collect()
    }

    /// Fetches all tokens that need to be updated sorted by priority.
    /// High-priority tokens (from `self.high_priority`) are returned first.
    fn prioritized_tokens_to_update(&self, max_age: Duration, now: Instant) -> Vec<Address> {
        let high_priority = self.high_priority.lock().unwrap();
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

    /// Helper for estimating a price with cache check and update.
    ///
    /// Returns early if a valid cached price exists, otherwise calls the
    /// provided fetch function and caches the result.
    ///
    /// This is the core logic shared by both on-demand price fetching and
    /// background maintenance.
    async fn estimate_with_cache_update<F, Fut>(
        &self,
        token: Address,
        require_updating_price: RequiresUpdatingPrices,
        keep_updated: KeepPriceUpdated,
        fetch: F,
    ) -> NativePriceEstimateResult
    where
        F: FnOnce(u32) -> Fut,
        Fut: std::future::Future<Output = NativePriceEstimateResult>,
    {
        let (current_accumulative_errors_count, existing_keep_updated) = {
            let now = Instant::now();
            let mut cache = self.cache.lock().unwrap();

            let cached = Self::lookup_cached_price(
                &mut cache,
                token,
                now,
                self.max_age,
                require_updating_price,
            );

            match cached {
                Some(cached) if cached.is_ready() => return cached.result,
                Some(cached) => (
                    cached.accumulative_errors_count,
                    cached.update_price_continuously,
                ),
                None => {
                    // Entry might exist but be expired - preserve its flag if so.
                    // If entry doesn't exist, use the caller's preference.
                    let existing_keep_updated = cache
                        .get(&token)
                        .map(|c| c.update_price_continuously)
                        .unwrap_or(KeepPriceUpdated::No);
                    (Default::default(), existing_keep_updated)
                }
            }
        };

        let result = fetch(current_accumulative_errors_count).await;

        if should_cache(&result) {
            let now = Instant::now();
            // Preserve Yes if existing entry had it, otherwise use the requested
            // keep_updated. This prevents downgrading auction-related tokens
            // when QuoteCompetitionEstimator requests them after expiration.
            let final_keep_updated = if existing_keep_updated == KeepPriceUpdated::Yes {
                KeepPriceUpdated::Yes
            } else {
                keep_updated
            };
            self.insert(
                token,
                CachedResult::new(
                    result.clone(),
                    now,
                    now,
                    current_accumulative_errors_count,
                    final_keep_updated,
                ),
            );
        }

        result
    }

    /// Estimates prices for the given tokens and updates the cache.
    /// Used by the background maintenance task. All tokens are processed using
    /// the provided estimator and marked with `KeepPriceUpdated::Yes`.
    ///
    /// This method batches lock acquisitions: one lock to get error counts,
    /// then concurrent fetches without locking, then one lock to insert
    /// results.
    async fn estimate_prices_and_update_cache_for_maintenance(
        &self,
        tokens: &[Address],
        estimator: &Arc<dyn NativePriceEstimating>,
        concurrent_requests: usize,
        request_timeout: Duration,
    ) -> usize {
        if tokens.is_empty() {
            return 0;
        }

        let error_counts = self.get_accumulative_errors(tokens);
        let futures: Vec<_> = tokens
            .iter()
            .map(|&token| {
                let estimator = estimator.clone();
                let token_to_fetch = *self.approximation_tokens.get(&token).unwrap_or(&token);
                let error_count = error_counts.get(&token).copied().unwrap_or_default();
                async move {
                    let result = estimator
                        .estimate_native_price(token_to_fetch, request_timeout)
                        .await;
                    (token, result, error_count)
                }
            })
            .collect();

        let results: Vec<_> = futures::stream::iter(futures)
            .buffered(concurrent_requests)
            .collect()
            .await;

        let now = Instant::now();
        let to_insert = results
            .iter()
            .filter(|(_, result, _)| should_cache(result))
            .map(|(token, result, error_count)| {
                (
                    *token,
                    CachedResult::new(
                        result.clone(),
                        now,
                        now,
                        *error_count,
                        KeepPriceUpdated::Yes,
                    ),
                )
            });
        self.insert_batch(to_insert);

        results.len()
    }
}

/// Spawns a background maintenance task for the given cache.
fn spawn_maintenance_task(cache: &Arc<CacheStorage>, config: MaintenanceConfig) {
    Metrics::get().reset();
    let update_task = CacheMaintenanceTask::new(Arc::downgrade(cache), config)
        .run()
        .instrument(tracing::info_span!("native_price_cache_maintenance"));
    tokio::spawn(update_task);
}

/// Background task that keeps the cache warm by periodically refreshing prices.
/// Only refreshes entries with `KeepPriceUpdated::Yes`; entries with
/// `KeepPriceUpdated::No` are cached but not maintained.
struct CacheMaintenanceTask {
    cache: Weak<CacheStorage>,
    /// Estimator used for maintenance updates.
    estimator: Arc<dyn NativePriceEstimating>,
    update_interval: Duration,
    update_size: usize,
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
    /// Only updates entries with `KeepPriceUpdated::Yes`.
    async fn single_update(&self, cache: &Arc<CacheStorage>) {
        let metrics = Metrics::get();
        metrics
            .native_price_cache_size
            .set(i64::try_from(cache.len()).unwrap_or(i64::MAX));

        let (maintained, passive) = cache.count_by_maintenance_flag();
        metrics
            .native_price_cache_maintained_entries
            .set(i64::try_from(maintained).unwrap_or_default());
        metrics
            .native_price_cache_passive_entries
            .set(i64::try_from(passive).unwrap_or_default());

        let max_age = cache.max_age().saturating_sub(self.prefetch_time);
        let mut outdated_entries = cache.prioritized_tokens_to_update(max_age, Instant::now());

        tracing::trace!(tokens = ?outdated_entries, first_n = ?self.update_size, "outdated auction prices to fetch");

        metrics
            .native_price_cache_outdated_entries
            .set(i64::try_from(outdated_entries.len()).unwrap_or(i64::MAX));

        if self.update_size > 0 {
            outdated_entries.truncate(self.update_size);
        }

        if outdated_entries.is_empty() {
            return;
        }

        let updates_count = cache
            .estimate_prices_and_update_cache_for_maintenance(
                &outdated_entries,
                &self.estimator,
                self.concurrent_requests,
                self.quote_timeout,
            )
            .await as u64;
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
    /// The `require_updating_prices` parameter controls whether entries fetched
    /// by this estimator should be actively maintained by the background task.
    pub fn new(
        estimator: Arc<dyn NativePriceEstimating>,
        cache: NativePriceCache,
        concurrent_requests: usize,
        require_updating_prices: RequiresUpdatingPrices,
    ) -> Self {
        Self {
            estimator,
            cache,
            concurrent_requests,
            require_updating_prices,
        }
    }

    /// Only returns prices that are currently cached. Missing prices will get
    /// prioritized to get fetched during the next cycles of the maintenance
    /// background task (only if `require_updating_prices == Yes`).
    ///
    /// If `require_updating_prices == Yes` and a cached entry has
    /// `KeepPriceUpdated::No`, it is upgraded to `KeepPriceUpdated::Yes`.
    fn get_cached_prices(
        &self,
        tokens: &[Address],
    ) -> HashMap<Address, Result<f64, PriceEstimationError>> {
        let now = Instant::now();
        let cached =
            self.cache
                .get_ready_to_use_cached_prices(tokens, now, self.require_updating_prices);

        cached
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
    /// If `require_updating_prices == Yes` and a cached entry has
    /// `KeepPriceUpdated::No`, it is upgraded to `KeepPriceUpdated::Yes`.
    fn estimate_prices_and_update_cache<'a>(
        &'a self,
        tokens: &'a [Address],
        request_timeout: Duration,
    ) -> futures::stream::BoxStream<'a, (Address, NativePriceEstimateResult)> {
        let keep_updated = match self.require_updating_prices {
            RequiresUpdatingPrices::Yes => KeepPriceUpdated::Yes,
            RequiresUpdatingPrices::DontCare => KeepPriceUpdated::No,
        };

        let estimates = tokens.iter().cloned().map(move |token| async move {
            let result = self
                .cache
                .estimate_with_cache_update(
                    token,
                    self.require_updating_prices,
                    keep_updated,
                    |_| self.fetch_price(token, request_timeout),
                )
                .await;
            (token, result)
        });
        futures::stream::iter(estimates)
            .buffered(self.concurrent_requests)
            .boxed()
    }

    /// Fetches a single price (without caching).
    async fn fetch_price(&self, token: Address, timeout: Duration) -> NativePriceEstimateResult {
        let token_to_fetch = *self
            .cache
            .approximation_tokens()
            .get(&token)
            .unwrap_or(&token);
        self.estimator
            .estimate_native_price(token_to_fetch, timeout)
            .await
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
            if let Some(cached) = self
                .cache
                .get_ready_to_use_cached_prices(&[token], now, self.require_updating_prices)
                .remove(&token)
            {
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

/// Wrapper around `CachingNativePriceEstimator` that marks all entries with
/// `KeepPriceUpdated::No`. Used for the autopilot API endpoints where prices
/// should be cached but not actively maintained by the background task.
#[derive(Clone)]
pub struct QuoteCompetitionEstimator(Arc<CachingNativePriceEstimator>);

impl QuoteCompetitionEstimator {
    /// Creates a new `QuoteCompetitionEstimator` wrapping the given estimator.
    ///
    /// Prices fetched through this wrapper will be cached with
    /// `KeepPriceUpdated::No`, meaning they won't be actively refreshed by the
    /// background maintenance task. However, if the same token is later
    /// requested with `RequiresUpdatingPrices::Yes`, the entry will be upgraded
    /// to `KeepPriceUpdated::Yes` and become actively maintained.
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
            // Don't upgrade or create entries, just read from cache
            if let Some(cached) = self
                .0
                .cache
                .get_ready_to_use_cached_prices(&[token], now, RequiresUpdatingPrices::DontCare)
                .remove(&token)
            {
                return cached.result;
            }

            // Cache the result but don't mark for active maintenance
            self.0
                .cache
                .estimate_with_cache_update(
                    token,
                    RequiresUpdatingPrices::DontCare,
                    KeepPriceUpdated::No,
                    |_| self.0.fetch_price(token, timeout),
                )
                .await
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
        let cache = CacheStorage::new_without_maintenance(
            Duration::from_secs(MAX_AGE_SECS),
            prices,
            Default::default(),
        );
        let estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            cache,
            1,
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
            CacheStorage::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
                Default::default(),
            ),
            1,
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
            // set token approximations for tokens 1 and 2
            CacheStorage::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
                HashMap::from([
                    (Address::with_last_byte(1), Address::with_last_byte(100)),
                    (Address::with_last_byte(2), Address::with_last_byte(200)),
                ]),
            ),
            1,
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
            CacheStorage::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
                Default::default(),
            ),
            1,
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
            CacheStorage::new_without_maintenance(
                Duration::from_millis(100),
                Default::default(),
                Default::default(),
            ),
            1,
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
            CacheStorage::new_without_maintenance(
                Duration::from_millis(30),
                Default::default(),
                Default::default(),
            ),
            1,
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
            Default::default(),
            MaintenanceConfig {
                estimator: Arc::new(maintenance),
                update_interval: Duration::from_millis(50),
                update_size: 1,
                prefetch_time: Default::default(),
                concurrent_requests: 1,
                quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
            },
        );

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(on_demand),
            cache,
            1,
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
            Default::default(),
            MaintenanceConfig {
                estimator: Arc::new(maintenance),
                update_interval: Duration::from_millis(50),
                update_size: 0,
                prefetch_time: Default::default(),
                concurrent_requests: 1,
                quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
            },
        );

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(on_demand),
            cache,
            1,
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
            Default::default(),
            MaintenanceConfig {
                estimator: Arc::new(maintenance),
                update_interval: Duration::from_millis(50),
                update_size: 0,
                prefetch_time: Default::default(),
                concurrent_requests: BATCH_SIZE,
                quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
            },
        );

        let estimator = CachingNativePriceEstimator::new(
            Arc::new(on_demand),
            cache,
            1,
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

        // Create a cache and populate it directly with `KeepPriceUpdated::Yes`
        // entries (since maintenance only updates those)
        let cache = CacheStorage::new_without_maintenance(
            Duration::from_secs(10),
            Default::default(),
            Default::default(),
        );
        cache.insert(
            t0,
            CachedResult::new(Ok(0.), now, now, Default::default(), KeepPriceUpdated::Yes),
        );
        cache.insert(
            t1,
            CachedResult::new(Ok(0.), now, now, Default::default(), KeepPriceUpdated::Yes),
        );

        let now = now + Duration::from_secs(1);

        cache.replace_high_priority(std::iter::once(t0).collect());
        let tokens = cache.prioritized_tokens_to_update(Duration::from_secs(0), now);
        assert_eq!(tokens[0], t0);
        assert_eq!(tokens[1], t1);

        cache.replace_high_priority(std::iter::once(t1).collect());
        let tokens = cache.prioritized_tokens_to_update(Duration::from_secs(0), now);
        assert_eq!(tokens[0], t1);
        assert_eq!(tokens[1], t0);
    }

    #[tokio::test]
    async fn quote_competition_estimator_preserves_keep_updated_yes() {
        // This test verifies that when QuoteCompetitionEstimator requests a token
        // that was previously marked with KeepPriceUpdated::Yes, the flag is preserved
        // even after the cache entry expires and needs to be re-fetched.

        let mut inner = MockNativePriceEstimating::new();
        // First call: auction-related estimator fetches the price
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| async { Ok(1.0) }.boxed());
        // Second call: QuoteCompetitionEstimator re-fetches after expiration
        inner
            .expect_estimate_native_price()
            .times(1)
            .returning(|_, _| async { Ok(2.0) }.boxed());

        let cache = CacheStorage::new_without_maintenance(
            Duration::from_millis(50),
            Default::default(),
            Default::default(),
        );

        // Create auction-related estimator (marks entries with KeepPriceUpdated::Yes)
        let auction_estimator = CachingNativePriceEstimator::new(
            Arc::new(inner),
            cache.clone(),
            1,
            RequiresUpdatingPrices::Yes,
        );

        // Create QuoteCompetitionEstimator (uses KeepPriceUpdated::No)
        let quote_estimator = QuoteCompetitionEstimator::new(Arc::new(auction_estimator));

        let t0 = token(0);

        // Step 1: Auction estimator fetches the price, marking it with Yes
        let result = quote_estimator
            .0
            .estimate_native_price(t0, HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.unwrap().to_i64().unwrap(), 1);

        // Verify the entry has KeepPriceUpdated::Yes
        {
            let cache_guard = cache.cache.lock().unwrap();
            let entry = cache_guard.get(&t0).unwrap();
            assert_eq!(entry.update_price_continuously, KeepPriceUpdated::Yes);
        }

        // Step 2: Wait for the cache entry to expire
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Step 3: QuoteCompetitionEstimator requests the same token (after expiration)
        // This would previously downgrade the entry to KeepPriceUpdated::No
        let result = quote_estimator
            .estimate_native_price(t0, HEALTHY_PRICE_ESTIMATION_TIME)
            .await;
        assert_eq!(result.unwrap().to_i64().unwrap(), 2);

        // Step 4: Verify the entry STILL has KeepPriceUpdated::Yes (not downgraded)
        {
            let cache_guard = cache.cache.lock().unwrap();
            let entry = cache_guard.get(&t0).unwrap();
            assert_eq!(
                entry.update_price_continuously,
                KeepPriceUpdated::Yes,
                "QuoteCompetitionEstimator should not downgrade KeepPriceUpdated::Yes to No"
            );
        }
    }
}
