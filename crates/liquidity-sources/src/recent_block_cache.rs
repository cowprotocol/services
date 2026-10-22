//! Module containing a generic cache for recent on-chain data.
//!
//! The design of this module is driven by the need to always return data
//! quickly so that end users going through the api do not have to wait longer
//! than necessary:
//! - The mutex is never locked while waiting on an async operation (getting
//!   on-chain data from the node).
//! - Automatically updating the cache is decoupled from normal on-chain data
//!   fetches.
//!
//! A result of this is that it is possible that the same uncached entry is
//! requested multiple times simultaneously and some work is wasted. This is
//! unlikely to happen in practice and the value is going to be cached the next
//! time it is needed.
//!
//! When entries are requested we mark all those entries as recently used which
//! potentially evicts other entries from the lru cache. Cache misses are
//! fetched and inserted into the cache. Then when the automatic update runs the
//! next time, we request and cache all recently used entries. For some
//! consumers we only care about the "recent" state of the entries. So we can
//! return any result from the cache even if it comes from previous blocks.
//!
//! On the other hand for others we need to fetch on-chain data at exact blocks
//! which is why we keep a cache of previous blocks in the first place as we
//! could simplify this module if it was only used by by the former.

use {
    alloy::eips::BlockId,
    anyhow::{Context, Result},
    cached::{Cached, TimedSizedCache},
    ethrpc::block_stream::CurrentBlockWatcher,
    futures::{FutureExt, StreamExt},
    itertools::Itertools,
    prometheus::IntCounterVec,
    request_sharing::BoxRequestSharing,
    std::{
        cmp,
        collections::{BTreeMap, HashMap, HashSet, hash_map::Entry},
        hash::Hash,
        num::NonZeroUsize,
        sync::{Arc, Mutex},
        time::Duration,
    },
    tracing::Instrument,
};

/// How many liqudity sources should at most be fetched in a single chunk.
const REQUEST_BATCH_SIZE: usize = 200;

/// A trait used to define `RecentBlockCache` updating behaviour.
#[async_trait::async_trait]
pub trait CacheFetching<K, V>: Send + Sync + 'static {
    async fn fetch_values(&self, keys: HashSet<K>, block: Block) -> Result<Vec<V>>;
}

/// A trait used for `RecentBlockCache` keys.
pub trait CacheKey<V>: Clone + Eq + Hash + Ord + Send + Sync + 'static {
    /// Returns the smallest possible value for this type's `std::cmp::Ord`
    /// implementation.
    fn first_ord() -> Self;

    /// Returns the key for the specified value.
    fn for_value(value: &V) -> Self;
}

/// The state of the chain at which information should be retrieved.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum Block {
    /// The most recent state. This is on a best effort basis so that for
    /// example a cache can still return results that are slightly out of
    /// date.
    Recent {
        /// Whether the requested entries should be kept up to date in the
        /// background so that repeated requests for them keep returning
        /// fresh state without fetching on the request path.
        schedule_update: bool,
    },
    Number(u64),
}

impl Block {
    /// Whether entries requested at this block get kept up to date by the
    /// background task.
    fn schedules_update(self) -> bool {
        matches!(
            self,
            // If a block number was specified the caller actually cared about the most accurate
            // data for these keys. In that case we want to be nice and remember the key for future
            // background updates of the cached liquidity.
            Block::Number(_)
                | Block::Recent {
                    schedule_update: true
                }
        )
    }
}

impl From<Block> for BlockId {
    fn from(value: Block) -> Self {
        match value {
            Block::Recent { .. } => BlockId::latest(),
            Block::Number(n) => BlockId::number(n),
        }
    }
}

/// Recent block cache for arbitrary key-value pairs.
///
/// Caches on-chain data for a specific number of blocks and automatically
/// updates the N most recently used entries automatically when a new block
/// arrives.
pub struct RecentBlockCache<K, V, F>
where
    K: CacheKey<V>,
    F: CacheFetching<K, V>,
{
    inner: Arc<Inner<K, V, F>>,
}

pub struct Inner<K, V, F>
where
    K: CacheKey<V>,
    F: CacheFetching<K, V>,
{
    mutexed: Mutex<Mutexed<K, V>>,
    fetcher: Arc<F>,
    maximum_retries: u32,
    retry_cooldown: Duration,
    metrics: &'static Metrics,
    metrics_label: &'static str,
    requests: BoxRequestSharing<(K, Block), Option<Vec<V>>>,
}

#[derive(Clone, Copy, Debug)]
pub struct CacheConfig {
    pub number_of_entries_to_auto_update: NonZeroUsize,
    pub auto_update_ttl: Duration,
    /// How many blocks older than the last background update a cached entry
    /// may be to still be returned for recent requests. Older entries get
    /// evicted by the background update.
    pub maximum_recent_block_age: u64,
    /// Time between two background updates of the recently used entries.
    /// Blocks arriving within that time share a single update.
    pub refresh_cooldown: Duration,
    pub max_retries: u32,
    /// Time to wait before retrying a failed fetch.
    pub retry_cooldown: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            number_of_entries_to_auto_update: NonZeroUsize::new(1).unwrap(),
            auto_update_ttl: Duration::from_secs(1),
            maximum_recent_block_age: Default::default(),
            refresh_cooldown: Default::default(),
            max_retries: Default::default(),
            retry_cooldown: Default::default(),
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// hits
    #[metric(labels("cache_type"))]
    recent_block_cache_hits: IntCounterVec,

    /// misses
    #[metric(labels("cache_type"))]
    recent_block_cache_misses: IntCounterVec,
}
impl<K, V, F> RecentBlockCache<K, V, F>
where
    K: CacheKey<V>,
    V: Clone + Send + Sync + 'static,
    F: CacheFetching<K, V>,
{
    /// number_of_entries_to_auto_update: The number of most recently used
    /// entries to keep track of and auto update when the current block
    /// changes.
    ///
    /// maximum_recent_block_age: When a recent block is requested, this is the
    /// maximum age a cached block can have to be considered. Older entries
    /// get evicted when the automatic updating runs the next time.
    pub fn new(
        config: CacheConfig,
        fetcher: F,
        block_stream: CurrentBlockWatcher,
        metrics_label: &'static str,
    ) -> Result<Self> {
        let block = block_stream.borrow().number;
        let inner = Arc::new(Inner {
            mutexed: Mutex::new(Mutexed::new(
                config.number_of_entries_to_auto_update,
                config.auto_update_ttl,
                block,
                config.maximum_recent_block_age,
            )),
            fetcher: Arc::new(fetcher),
            maximum_retries: config.max_retries,
            retry_cooldown: config.retry_cooldown,
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
            metrics_label,
            requests: BoxRequestSharing::labelled("liquidity_fetching".into()),
        });

        Self::spawn_gc_task(
            Arc::downgrade(&inner),
            block_stream,
            config.refresh_cooldown,
            metrics_label.to_string(),
        );

        Ok(Self { inner })
    }

    pub async fn fetch(&self, keys: impl IntoIterator<Item = K>, block: Block) -> Result<Vec<V>> {
        self.inner.fetch(keys, block).await
    }

    fn spawn_gc_task(
        inner: std::sync::Weak<Inner<K, V, F>>,
        block_stream: CurrentBlockWatcher,
        refresh_cooldown: Duration,
        label: String,
    ) {
        tokio::task::spawn(async move {
            let mut stream = ethrpc::block_stream::into_stream(block_stream);
            // `interval` panics on a zero period.
            let mut cooldown =
                tokio::time::interval(refresh_cooldown.max(Duration::from_millis(1)));
            cooldown.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            // The first tick completes immediately; consume it so the first
            // update is followed by a cooldown too.
            cooldown.tick().await;
            while let Some(block) = stream.next().await {
                let Some(inner) = inner.upgrade() else {
                    tracing::debug!("cache no longer in use; terminate GC task");
                    break;
                };
                if let Err(err) = inner
                    .update_cache_at_block(block.number)
                    .instrument(tracing::info_span!("cache_maintenance", cache = %label))
                    .await
                {
                    tracing::warn!(?err, "failed to update cache");
                }
                // The stream only yields the newest block once we continue, so
                // blocks arriving during the cooldown collapse into a single
                // update instead of refreshing every entry once per block. The
                // cooldown runs from tick to tick, so the update itself counts
                // towards it and on average there is at most one update per
                // cooldown.
                cooldown.tick().await;
            }
        });
    }
}

impl<K, V, F> Inner<K, V, F>
where
    K: CacheKey<V>,
    V: Clone + Send + Sync + 'static,
    F: CacheFetching<K, V>,
{
    async fn update_cache_at_block(&self, new_block: u64) -> Result<()> {
        let keys = self
            .mutexed
            .lock()
            .unwrap()
            .keys_of_recently_used_entries()
            .collect::<HashSet<_>>();
        tracing::debug!("automatically updating {} entries", keys.len());
        let found_values = self
            .fetch_inner_many(keys.clone(), Block::Number(new_block))
            .await?;

        let mut mutexed = self.mutexed.lock().unwrap();
        mutexed.insert(new_block, keys, found_values);
        // Entries older than that are no longer returned for recent requests,
        // so there is no point in keeping them around.
        let oldest_to_keep = new_block.saturating_sub(mutexed.maximum_recent_block_age);
        mutexed.remove_cached_blocks_older_than(oldest_to_keep);
        mutexed.last_update_block = new_block;

        Ok(())
    }

    async fn fetch_inner_many(&self, keys: HashSet<K>, block: Block) -> Result<Vec<V>> {
        let fetched =
            futures::future::join_all(keys.iter().map(|key| self.fetch_inner(key.clone(), block)))
                .await;
        let fetched: Vec<_> = fetched
            .into_iter()
            .filter_map(|res| res.ok())
            .flatten()
            .collect();
        Ok(fetched)
    }

    // Sometimes nodes requests error when we try to get state from what we
    // think is the current block when the node has been load balanced out
    // to one that hasn't seen the block yet. As a workaround we repeat the
    // request up to N times while sleeping in between.
    async fn fetch_inner(&self, key: K, block: Block) -> Result<Vec<V>> {
        let retries = self.maximum_retries;
        let delay = self.retry_cooldown;
        let fetcher = self.fetcher.clone();
        let shared = self.requests.shared_or_else((key, block), |entry| {
            let (key, block) = entry.clone();
            async move {
                for _ in 0..=retries {
                    let keys = [key.clone()].into();
                    match fetcher.fetch_values(keys, block).await {
                        Ok(values) => return Some(values),
                        Err(err) => tracing::warn!("retrying fetch because error: {:?}", err),
                    }
                    tokio::time::sleep(delay).await;
                }
                None
            }
            .boxed()
        });
        shared.await.context("could not fetch liquidity")
    }

    async fn fetch(&self, keys: impl IntoIterator<Item = K>, block: Block) -> Result<Vec<V>> {
        let block_number = match block {
            Block::Recent { .. } => None,
            Block::Number(number) => Some(number),
        };

        let mut cache_hit_count = 0usize;
        let mut cache_hits = Vec::new();
        let mut cache_misses = HashSet::new();
        let last_update_block;
        {
            let mut mutexed = self.mutexed.lock().unwrap();
            for key in keys {
                match mutexed.get(key.clone(), block) {
                    Some(values) => {
                        cache_hit_count += 1;
                        cache_hits.extend_from_slice(values);
                    }
                    None => {
                        cache_misses.insert(key);
                    }
                }
            }
            last_update_block = mutexed.last_update_block;
        }

        self.metrics
            .recent_block_cache_hits
            .with_label_values(&[self.metrics_label])
            .inc_by(cache_hit_count as u64);
        self.metrics
            .recent_block_cache_misses
            .with_label_values(&[self.metrics_label])
            .inc_by(cache_misses.len() as u64);

        if cache_misses.is_empty() {
            return Ok(cache_hits);
        }

        let cache_miss_block = block_number.unwrap_or(last_update_block);
        let cache_misses: Vec<_> = cache_misses.into_iter().collect();
        // Splits fetches into chunks because we can get over 1400 requests when
        // the cache is empty which tend to time out if we don't chunk
        // them.
        for chunk in cache_misses.chunks(REQUEST_BATCH_SIZE) {
            let keys = chunk.iter().cloned().collect();
            let fetched = self
                .fetch_inner_many(keys, Block::Number(cache_miss_block))
                .await?;
            let found_keys = fetched.iter().map(K::for_value).unique().collect_vec();
            cache_hits.extend_from_slice(&fetched);

            let mut mutexed = self.mutexed.lock().unwrap();
            mutexed.insert(cache_miss_block, chunk.iter().cloned(), fetched);
            if block.schedules_update() {
                for key in found_keys {
                    mutexed.recently_used.cache_set(key, ());
                }
            }
        }

        Ok(cache_hits)
    }
}

#[derive(Debug)]
struct Mutexed<K, V>
where
    K: CacheKey<V>,
{
    recently_used: TimedSizedCache<K, ()>,
    // For quickly finding at which block an entry is cached.
    cached_most_recently_at_block: HashMap<K, u64>,
    // Tuple ordering allows us to efficiently construct range queries by block.
    entries: BTreeMap<(u64, K), Vec<V>>,
    // The last block at which the automatic cache updating happened.
    last_update_block: u64,
    // Maximum age a cached block can have to count as recent.
    maximum_recent_block_age: u64,
}

impl<K, V> Mutexed<K, V>
where
    K: CacheKey<V>,
{
    fn new(
        entries_lru_size: NonZeroUsize,
        entries_ttl: Duration,
        current_block: u64,
        maximum_recent_block_age: u64,
    ) -> Self {
        Self {
            recently_used: TimedSizedCache::with_size_and_lifespan(
                entries_lru_size.get(),
                entries_ttl.as_secs(),
            ),
            cached_most_recently_at_block: HashMap::new(),
            entries: BTreeMap::new(),
            last_update_block: current_block,
            maximum_recent_block_age,
        }
    }

    /// Returns the values cached for `key` at the requested block. Without a
    /// specific block number the most recently cached values are returned as
    /// long as they are not too old compared to the last background update.
    fn get(&mut self, key: K, block: Block) -> Option<&[V]> {
        let cached_at = match block {
            Block::Number(number) => number,
            Block::Recent { .. } => self
                .cached_most_recently_at_block
                .get(&key)
                .copied()
                .filter(|&cached_at| {
                    self.last_update_block.saturating_sub(cached_at)
                        <= self.maximum_recent_block_age
                })?,
        };
        let result = self
            .entries
            .get(&(cached_at, key.clone()))
            .map(Vec::as_slice);
        if block.schedules_update() && result.is_some_and(|values| !values.is_empty()) {
            self.recently_used.cache_set(key, ());
        }
        result
    }

    fn insert(
        &mut self,
        block: u64,
        keys: impl IntoIterator<Item = K>,
        values: impl IntoIterator<Item = V>,
    ) {
        for key in keys {
            match self.cached_most_recently_at_block.entry(key.clone()) {
                Entry::Occupied(mut entry) => {
                    let value = entry.get_mut();
                    *value = cmp::max(*value, block);
                }
                Entry::Vacant(entry) => {
                    entry.insert(block);
                }
            }
            // Make sure entries without any values are cached.
            self.entries.insert((block, key), Vec::new());
        }
        for value in values {
            // Unwrap because previous loop guarantees all keys have an entry.
            self.entries
                .get_mut(&(block, K::for_value(&value)))
                .unwrap()
                .push(value);
        }
    }

    fn remove_cached_blocks_older_than(&mut self, oldest_to_keep: u64) {
        tracing::debug!("dropping blocks older than {} from cache", oldest_to_keep);
        self.entries = self.entries.split_off(&(oldest_to_keep, K::first_ord()));

        // Iterate from newest block to oldest block and only keep the most
        // recent liquidity around to reduce memory consumption.
        let mut cached_keys = HashSet::new();
        let mut items = 0;
        for ((_block, key), values) in self.entries.iter_mut().rev() {
            if !cached_keys.insert(key) {
                *values = vec![];
            } else {
                items += values.len();
            }
        }
        // Afterwards drop all entries that are now empty.
        self.entries.retain(|_, values| !values.is_empty());

        self.cached_most_recently_at_block
            .retain(|_, block| *block >= oldest_to_keep);
        tracing::debug!(
            entries = self.entries.len(),
            items,
            "cache was updated and now contains",
        );
    }

    fn keys_of_recently_used_entries(&self) -> impl Iterator<Item = K> + '_ {
        self.recently_used.key_order().cloned()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ethrpc::block_stream::{BlockInfo, mock_single_block},
        futures::FutureExt,
        std::sync::Arc,
    };

    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    struct TestKey(usize);

    impl CacheKey<TestValue> for TestKey {
        fn first_ord() -> Self {
            Self(0)
        }

        fn for_value(value: &TestValue) -> Self {
            Self(value.key)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TestValue {
        key: usize,
        data: String,
    }

    impl TestValue {
        fn new(key: usize, data: impl Into<String>) -> Self {
            Self {
                key,
                data: data.into(),
            }
        }
    }

    #[derive(Default)]
    struct FakeCacheFetcher(Arc<Mutex<Vec<TestValue>>>);

    #[async_trait::async_trait]
    impl CacheFetching<TestKey, TestValue> for FakeCacheFetcher {
        async fn fetch_values(
            &self,
            requested: HashSet<TestKey>,
            _: Block,
        ) -> Result<Vec<TestValue>> {
            let fetched = self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|value| requested.contains(&TestKey(value.key)))
                .cloned()
                .collect();
            Ok(fetched)
        }
    }

    impl FakeCacheFetcher {
        pub fn new(values: Vec<TestValue>) -> Self {
            Self(Arc::new(Mutex::new(values)))
        }
    }

    fn test_keys(keys: impl IntoIterator<Item = usize>) -> impl Iterator<Item = TestKey> {
        keys.into_iter().map(TestKey)
    }

    /// Recent state without scheduling the entries for background updates.
    const RECENT: Block = Block::Recent {
        schedule_update: false,
    };

    #[tokio::test]
    async fn marks_recently_used() {
        let fetcher = FakeCacheFetcher::new(vec![
            TestValue::new(0, "a"),
            TestValue::new(1, "b"),
            // no liquidity for key 2 on-chain
            TestValue::new(3, "c"),
        ]);
        let block_number = 10u64;
        let block_stream = mock_single_block(BlockInfo {
            number: block_number,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                number_of_entries_to_auto_update: NonZeroUsize::new(1).unwrap(),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;

        let assert_keys_recently_used = |expected_keys: &[usize]| {
            let cached_keys = cache
                .mutexed
                .lock()
                .unwrap()
                .keys_of_recently_used_entries()
                .collect::<Vec<_>>();
            let expected_keys: Vec<_> = expected_keys.iter().copied().map(TestKey).collect();
            assert_eq!(cached_keys, expected_keys);
        };

        cache
            .fetch(test_keys(0..1), Block::Number(block_number))
            .await
            .unwrap();
        assert_keys_recently_used(&[0]);

        // Don't cache this because we didn't request the liquidity on a
        // specific block.
        cache.fetch(test_keys(1..2), RECENT).await.unwrap();
        assert_keys_recently_used(&[0]);

        // Don't cache this because there is no liquidity for this block
        // on-chain.
        cache
            .fetch(test_keys(2..3), Block::Number(block_number))
            .await
            .unwrap();
        assert_keys_recently_used(&[0]);

        // Cache the new key but evict the other key because we have a limited
        // capacity.
        cache
            .fetch(test_keys(3..4), Block::Number(block_number))
            .await
            .unwrap();
        assert_keys_recently_used(&[3]);
    }

    #[tokio::test]
    async fn auto_updates_recently_used() {
        let fetcher = FakeCacheFetcher::default();
        let values = fetcher.0.clone();
        let block_number = 10u64;
        let block_stream = mock_single_block(BlockInfo {
            number: block_number,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                number_of_entries_to_auto_update: NonZeroUsize::new(4).unwrap(),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;

        // Initial state on the block chain.
        let initial_values = vec![
            TestValue::new(0, "1"),
            TestValue::new(1, "1"),
            TestValue::new(2, "1"),
            TestValue::new(3, "1"),
        ];
        values.lock().unwrap().clone_from(&initial_values);

        let result = cache
            .fetch(test_keys(0..2), Block::Number(block_number))
            .await
            .unwrap();
        assert_eq!(result.len(), 2);

        let result = cache.fetch(test_keys(0..4), RECENT).await.unwrap();
        // We can fetch data for keys with `Recent` but we don't schedule them
        // for auto updates.
        assert_eq!(result.len(), 4);

        // New state on the block chain on the next block.
        let updated_values = vec![
            TestValue::new(0, "2"),
            TestValue::new(1, "2"),
            TestValue::new(2, "2"),
            TestValue::new(3, "2"),
        ];
        values.lock().unwrap().clone_from(&updated_values);
        cache.update_cache_at_block(block_number).await.unwrap();
        values.lock().unwrap().clear();

        let result = cache.fetch(test_keys(0..4), RECENT).await.unwrap();
        assert_eq!(result.len(), 4);
        // These keys were scheduled for background updates and show the new
        // value.
        assert!(result.contains(&updated_values[0]));
        assert!(result.contains(&updated_values[1]));
        // These keys were NOT scheduled for background updates and show the old
        // value.
        assert!(result.contains(&initial_values[2]));
        assert!(result.contains(&initial_values[3]));
    }

    #[tokio::test]
    async fn cache_hit_and_miss() {
        let fetcher = FakeCacheFetcher::default();
        let values = fetcher.0.clone();
        let block_number = 10u64;
        let block_stream = mock_single_block(BlockInfo {
            number: block_number,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                number_of_entries_to_auto_update: NonZeroUsize::new(2).unwrap(),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;

        let value0 = TestValue::new(0, "0");
        let value1 = TestValue::new(1, "1");
        let value2 = TestValue::new(2, "2");

        *values.lock().unwrap() = vec![value0.clone(), value1.clone()];
        // cache miss gets cached
        cache
            .fetch(test_keys(0..2), RECENT)
            .now_or_never()
            .unwrap()
            .unwrap();

        *values.lock().unwrap() = vec![value2.clone()];
        // key 1 is cache hit, key 2 is miss
        let result = cache
            .fetch(test_keys(1..3), RECENT)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&value1));
        assert!(result.contains(&value2));

        // Make sure everything is still properly cached.
        values.lock().unwrap().clear();
        let result = cache
            .fetch(test_keys(0..3), RECENT)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&value0));
        assert!(result.contains(&value1));
        assert!(result.contains(&value2));
    }

    #[tokio::test]
    async fn uses_most_recent_cached_for_latest_block() {
        let fetcher = FakeCacheFetcher::default();
        let values = fetcher.0.clone();
        let block_number = 10u64;
        let block_stream = mock_single_block(BlockInfo {
            number: block_number,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                number_of_entries_to_auto_update: NonZeroUsize::new(2).unwrap(),
                maximum_recent_block_age: 10,
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;

        // cache at block 5
        *values.lock().unwrap() = vec![TestValue::new(0, "foo")];
        let result = cache
            .fetch(test_keys(0..1), Block::Number(5))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "foo")]);

        // cache at block 6
        *values.lock().unwrap() = vec![TestValue::new(0, "bar")];
        let result = cache
            .fetch(test_keys(0..1), Block::Number(6))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "bar")]);

        values.lock().unwrap().clear();
        // cache hit at block 6
        let result = cache
            .fetch(test_keys(0..1), RECENT)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "bar")]);

        // Now cache at an earlier block and see that it doesn't override the
        // most recent entry.
        *values.lock().unwrap() = vec![TestValue::new(0, "baz")];
        let result = cache
            .fetch(test_keys(0..1), Block::Number(4))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "baz")]);

        // We still get the cache hit from block 6.
        let result = cache
            .fetch(test_keys(0..1), RECENT)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "bar")]);
    }

    #[tokio::test]
    async fn evicts_old_blocks_from_cache() {
        let values = (0..=12).map(|key| TestValue::new(key, "")).collect();
        let fetcher = FakeCacheFetcher::new(values);
        let block = |number| BlockInfo {
            number,
            ..Default::default()
        };
        let (block_sender, block_stream) = tokio::sync::watch::channel(block(10));
        let cache = RecentBlockCache::new(
            CacheConfig {
                maximum_recent_block_age: 1,
                number_of_entries_to_auto_update: NonZeroUsize::new(2).unwrap(),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;

        // Fetch 10 keys on block 10; but we only have capacity to update 2 of
        // those in the background.
        cache
            .fetch(test_keys(0..10), Block::Number(10))
            .await
            .unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 10);

        block_sender.send(block(11)).unwrap();
        // Fetch updated liquidity for 2 of the initial 10 keys
        cache.update_cache_at_block(11).await.unwrap();
        // Fetch 2 new keys which are NOT scheduled for background updates
        cache.fetch(test_keys(10..12), RECENT).await.unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 12);

        block_sender.send(block(12)).unwrap();
        // Fetch updated liquidity for 2 of the initial 10 keys
        cache.update_cache_at_block(12).await.unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 4);

        block_sender.send(block(13)).unwrap();
        // Update 2 blocks in background but now it's time to evict the 2
        // additional keys we fetched with `Block::Recent` because we
        // are only allowed to keep state that is up to 1 block old.
        cache.update_cache_at_block(13).await.unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 2);
    }

    #[tokio::test]
    async fn respects_max_age_limit_for_recent() {
        let fetcher = FakeCacheFetcher::default();
        let block_number = 10u64;
        let block_stream = mock_single_block(BlockInfo {
            number: block_number,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                maximum_recent_block_age: 2,
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;
        let key = TestKey(0);

        // cache at block 7, most recent block is 10.
        cache
            .fetch(std::iter::once(key), Block::Number(7))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(
            cache
                .mutexed
                .lock()
                .unwrap()
                .get(key, Block::Number(7))
                .is_some()
        );
        assert!(cache.mutexed.lock().unwrap().get(key, RECENT).is_none());

        // cache at block 8
        cache
            .fetch(std::iter::once(key), Block::Number(8))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(
            cache
                .mutexed
                .lock()
                .unwrap()
                .get(key, Block::Number(7))
                .is_some()
        );
        assert!(
            cache
                .mutexed
                .lock()
                .unwrap()
                .get(key, Block::Number(8))
                .is_some()
        );
        assert!(cache.mutexed.lock().unwrap().get(key, RECENT).is_some());
    }

    #[tokio::test]
    async fn recent_with_schedule_update_tracks_entries() {
        let fetcher = FakeCacheFetcher::default();
        let values = fetcher.0.clone();
        let block_stream = mock_single_block(BlockInfo {
            number: 10,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                number_of_entries_to_auto_update: NonZeroUsize::new(2).unwrap(),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;
        let tracked_keys = || {
            let mut keys = cache
                .mutexed
                .lock()
                .unwrap()
                .keys_of_recently_used_entries()
                .collect::<Vec<_>>();
            keys.sort();
            keys
        };

        *values.lock().unwrap() = vec![TestValue::new(0, "a"), TestValue::new(1, "a")];
        // `Recent` caches the entry but doesn't schedule it for updates.
        let result = cache
            .fetch(
                test_keys(0..1),
                Block::Recent {
                    schedule_update: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "a")]);
        assert!(tracked_keys().is_empty());

        // The cache hit (key 0) and the cache miss (key 1) both get scheduled.
        let result = cache
            .fetch(
                test_keys(0..2),
                Block::Recent {
                    schedule_update: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(tracked_keys(), vec![TestKey(0), TestKey(1)]);

        // The background update refreshes both without any request.
        *values.lock().unwrap() = vec![TestValue::new(0, "b"), TestValue::new(1, "b")];
        cache.update_cache_at_block(11).await.unwrap();
        values.lock().unwrap().clear();
        let result = cache
            .fetch(
                test_keys(0..2),
                Block::Recent {
                    schedule_update: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&TestValue::new(0, "b")));
        assert!(result.contains(&TestValue::new(1, "b")));
    }

    #[tokio::test(start_paused = true)]
    async fn refresh_cooldown_collapses_blocks_into_one_update() {
        let fetcher = FakeCacheFetcher::new(vec![TestValue::new(0, "")]);
        let block = |number| BlockInfo {
            number,
            ..Default::default()
        };
        let (block_sender, block_stream) = tokio::sync::watch::channel(block(10));
        let cache = RecentBlockCache::new(
            CacheConfig {
                refresh_cooldown: Duration::from_secs(10),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;
        let last_update_block = || cache.mutexed.lock().unwrap().last_update_block;
        let cached_blocks = || {
            cache
                .mutexed
                .lock()
                .unwrap()
                .entries
                .keys()
                .map(|(block, _)| *block)
                .collect::<Vec<_>>()
        };

        // Schedule the key for background updates.
        cache
            .fetch(test_keys(0..1), Block::Number(10))
            .await
            .unwrap();
        // Let the GC task process the initial block and start its cooldown.
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(last_update_block(), 10);

        // Blocks arriving during the cooldown don't trigger updates.
        for number in 11..=13 {
            block_sender.send(block(number)).unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        assert_eq!(last_update_block(), 10);
        assert_eq!(cached_blocks(), vec![10]);

        // Once the cooldown is over only the newest block gets processed.
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(last_update_block(), 13);
        assert_eq!(cached_blocks(), vec![13]);
    }

    #[tokio::test(start_paused = true)]
    async fn refresh_cooldown_includes_update_duration() {
        struct SlowFetcher(Duration);

        #[async_trait::async_trait]
        impl CacheFetching<TestKey, TestValue> for SlowFetcher {
            async fn fetch_values(
                &self,
                keys: HashSet<TestKey>,
                _: Block,
            ) -> Result<Vec<TestValue>> {
                tokio::time::sleep(self.0).await;
                Ok(keys
                    .into_iter()
                    .map(|key| TestValue::new(key.0, ""))
                    .collect())
            }
        }

        let block = |number| BlockInfo {
            number,
            ..Default::default()
        };
        let (block_sender, block_stream) = tokio::sync::watch::channel(block(10));
        let cache = RecentBlockCache::new(
            CacheConfig {
                refresh_cooldown: Duration::from_secs(10),
                ..Default::default()
            },
            SlowFetcher(Duration::from_secs(4)),
            block_stream,
            "",
        )
        .unwrap()
        .inner;
        let last_update_block = || cache.mutexed.lock().unwrap().last_update_block;

        // Schedule the key for background updates. Meanwhile the GC task
        // processes the initial block (no entries yet) and starts its cooldown
        // at t=0.
        cache
            .fetch(test_keys(0..1), Block::Number(10))
            .await
            .unwrap();

        // t=4: the next update starts at t=10 and takes until t=14.
        block_sender.send(block(11)).unwrap();
        tokio::time::sleep(Duration::from_secs(12)).await;
        assert_eq!(last_update_block(), 11);

        // t=16: the cooldown started at t=10, so the next update starts at
        // t=20 and finishes at t=24 instead of waiting until t=24 to start.
        block_sender.send(block(12)).unwrap();
        tokio::time::sleep(Duration::from_secs(9)).await;
        assert_eq!(last_update_block(), 12);
    }
}
