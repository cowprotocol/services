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
    crate::current_block::CurrentBlockStream,
    anyhow::Result,
    ethcontract::BlockNumber,
    lru::LruCache,
    prometheus::IntCounterVec,
    std::{
        cmp,
        collections::{hash_map::Entry, BTreeMap, HashMap, HashSet},
        hash::Hash,
        num::{NonZeroU64, NonZeroUsize},
        sync::Mutex,
        time::Duration,
    },
};

/// A trait used to define `RecentBlockCache` updating behaviour.
#[async_trait::async_trait]
pub trait CacheFetching<K, V> {
    async fn fetch_values(&self, keys: HashSet<K>, block: Block) -> Result<Vec<V>>;
}

/// A trait used for `RecentBlockCache` keys.
pub trait CacheKey<V>: Clone + Eq + Hash + Ord {
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
    Recent,
    Number(u64),
}

impl From<Block> for BlockNumber {
    fn from(val: Block) -> Self {
        match val {
            Block::Recent => BlockNumber::Latest,
            Block::Number(number) => BlockNumber::Number(number.into()),
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
    mutexed: Mutex<Mutexed<K, V>>,
    number_of_blocks_to_cache: NonZeroU64,
    fetcher: F,
    block_stream: CurrentBlockStream,
    maximum_retries: u32,
    delay_between_retries: Duration,
    metrics: &'static Metrics,
    metrics_label: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct CacheConfig {
    pub number_of_blocks_to_cache: NonZeroU64,
    pub number_of_entries_to_auto_update: NonZeroUsize,
    pub maximum_recent_block_age: u64,
    pub max_retries: u32,
    pub delay_between_retries: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            number_of_blocks_to_cache: NonZeroU64::new(1).unwrap(),
            number_of_entries_to_auto_update: NonZeroUsize::new(1).unwrap(),
            maximum_recent_block_age: Default::default(),
            max_retries: Default::default(),
            delay_between_retries: Default::default(),
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
    V: Clone,
    F: CacheFetching<K, V>,
{
    /// number_of_blocks_to_cache: Previous blocks stay cached until the block
    /// is this much older than the current block. If there is a request for
    /// a block that is already too old then the result stays cached until
    /// the automatic updating runs the next time.
    ///
    /// number_of_entries_to_auto_update: The number of most recently used
    /// entries to keep track of and auto update when the current block
    /// changes.
    ///
    /// maximum_recent_block_age: When a recent block is requested, this is the
    /// maximum a cached block can have to be considered.
    pub fn new(
        config: CacheConfig,
        fetcher: F,
        block_stream: CurrentBlockStream,
        metrics_label: &'static str,
    ) -> Result<Self> {
        let block = block_stream.borrow().number;
        Ok(Self {
            mutexed: Mutex::new(Mutexed::new(
                config.number_of_entries_to_auto_update,
                block,
                config.maximum_recent_block_age,
            )),
            number_of_blocks_to_cache: config.number_of_blocks_to_cache,
            fetcher,
            block_stream,
            maximum_retries: config.max_retries,
            delay_between_retries: config.delay_between_retries,
            metrics: Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
            metrics_label,
        })
    }

    pub async fn update_cache(&self) -> Result<()> {
        let new_block = self.block_stream.borrow().number;
        self.update_cache_at_block(new_block).await
    }

    async fn update_cache_at_block(&self, new_block: u64) -> Result<()> {
        let keys = self
            .mutexed
            .lock()
            .unwrap()
            .keys_of_recently_used_entries()
            .collect::<HashSet<_>>();
        tracing::debug!("automatically updating {} entries", keys.len());
        let entries = self
            .fetch_inner(keys.clone(), Block::Number(new_block))
            .await?;
        {
            let mut mutexed = self.mutexed.lock().unwrap();
            mutexed.insert(new_block, keys.into_iter(), entries);
            let oldest_to_keep = new_block.saturating_sub(self.number_of_blocks_to_cache.get() - 1);
            mutexed.remove_cached_blocks_older_than(oldest_to_keep);
            mutexed.last_update_block = new_block;
        }
        Ok(())
    }

    // Sometimes nodes requests error when we try to get state from what we think is
    // the current block when the node has been load balanced out to one that
    // hasn't seen the block yet. As a workaround we repeat the request up to N
    // times while sleeping in between.
    async fn fetch_inner(&self, keys: HashSet<K>, block: Block) -> Result<Vec<V>> {
        let fetch = || self.fetcher.fetch_values(keys.clone(), block);
        for _ in 0..self.maximum_retries {
            match fetch().await {
                Ok(values) => return Ok(values),
                Err(err) => tracing::warn!("retrying fetch because error: {:?}", err),
            }
            tokio::time::sleep(self.delay_between_retries).await;
        }
        fetch().await
    }

    pub async fn fetch(&self, keys: impl IntoIterator<Item = K>, block: Block) -> Result<Vec<V>> {
        let block = match block {
            Block::Recent => None,
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

        let cache_miss_block = block.unwrap_or(last_update_block);
        let uncached_values = self
            .fetch_inner(cache_misses.clone(), Block::Number(cache_miss_block))
            .await?;

        cache_hits.extend_from_slice(&uncached_values);

        {
            let mut mutexed = self.mutexed.lock().unwrap();
            mutexed.insert(cache_miss_block, cache_misses.into_iter(), uncached_values);
        }

        Ok(cache_hits)
    }
}

#[derive(Debug)]
struct Mutexed<K, V>
where
    K: CacheKey<V>,
{
    recently_used: LruCache<K, ()>,
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
        current_block: u64,
        maximum_recent_block_age: u64,
    ) -> Self {
        Self {
            recently_used: LruCache::new(entries_lru_size),
            cached_most_recently_at_block: HashMap::new(),
            entries: BTreeMap::new(),
            last_update_block: current_block,
            maximum_recent_block_age,
        }
    }

    fn get(&mut self, key: K, block: Option<u64>) -> Option<&[V]> {
        self.recently_used.put(key.clone(), ());
        let block = block.or_else(|| {
            self.cached_most_recently_at_block
                .get(&key)
                .copied()
                .filter(|&block| {
                    self.last_update_block.saturating_sub(block) <= self.maximum_recent_block_age
                })
        })?;
        self.entries.get(&(block, key)).map(Vec::as_slice)
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
        self.cached_most_recently_at_block
            .retain(|_, block| *block >= oldest_to_keep);
        tracing::debug!(
            "the cache now contains entries for {} block-key combinations",
            self.entries.len()
        );
    }

    fn keys_of_recently_used_entries(&self) -> impl Iterator<Item = K> + '_ {
        self.recently_used.iter().map(|(key, _)| key.clone())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::current_block::{self, BlockInfo},
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
        async fn fetch_values(&self, _: HashSet<TestKey>, _: Block) -> Result<Vec<TestValue>> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    fn test_keys(keys: impl IntoIterator<Item = usize>) -> impl Iterator<Item = TestKey> {
        keys.into_iter().map(TestKey)
    }

    #[test]
    fn marks_recently_used() {
        let fetcher = FakeCacheFetcher::default();
        let block_number = 10u64;
        let block_stream = current_block::mock_single_block(BlockInfo {
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
        .unwrap();

        cache
            .fetch(test_keys(0..1), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        cache
            .fetch(test_keys(1..2), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        let keys = cache
            .mutexed
            .lock()
            .unwrap()
            .keys_of_recently_used_entries()
            .collect::<HashSet<_>>();
        assert_eq!(keys, test_keys(0..2).collect());

        // 1 is already cached, 3 isn't.
        cache
            .fetch(test_keys(1..3), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        let keys = cache
            .mutexed
            .lock()
            .unwrap()
            .keys_of_recently_used_entries()
            .collect::<HashSet<_>>();
        assert_eq!(keys, test_keys(1..3).collect());
    }

    #[test]
    fn auto_updates_recently_used() {
        let fetcher = FakeCacheFetcher::default();
        let values = fetcher.0.clone();
        let block_number = 10u64;
        let block_stream = current_block::mock_single_block(BlockInfo {
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
        .unwrap();

        let result = cache
            .fetch(test_keys(0..2), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(result.is_empty());

        let updated_values = vec![TestValue::new(0, "hello"), TestValue::new(1, "ether")];
        *values.lock().unwrap() = updated_values.clone();
        cache
            .update_cache_at_block(block_number)
            .now_or_never()
            .unwrap()
            .unwrap();
        values.lock().unwrap().clear();

        let result = cache
            .fetch(test_keys(0..2), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 2);
        for value in updated_values {
            assert!(result.contains(&value));
        }
    }

    #[test]
    fn cache_hit_and_miss() {
        let fetcher = FakeCacheFetcher::default();
        let values = fetcher.0.clone();
        let block_number = 10u64;
        let block_stream = current_block::mock_single_block(BlockInfo {
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
        .unwrap();

        let value0 = TestValue::new(0, "0");
        let value1 = TestValue::new(1, "1");
        let value2 = TestValue::new(2, "2");

        *values.lock().unwrap() = vec![value0.clone(), value1.clone()];
        // cache miss gets cached
        cache
            .fetch(test_keys(0..2), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();

        *values.lock().unwrap() = vec![value2.clone()];
        // key 1 is cache hit, key 2 is miss
        let result = cache
            .fetch(test_keys(1..3), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&value1));
        assert!(result.contains(&value2));

        // Make sure everything is still properly cached.
        values.lock().unwrap().clear();
        let result = cache
            .fetch(test_keys(0..3), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&value0));
        assert!(result.contains(&value1));
        assert!(result.contains(&value2));
    }

    #[test]
    fn uses_most_recent_cached_for_latest_block() {
        let fetcher = FakeCacheFetcher::default();
        let values = fetcher.0.clone();
        let block_number = 10u64;
        let block_stream = current_block::mock_single_block(BlockInfo {
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
        .unwrap();

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
            .fetch(test_keys(0..1), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "bar")]);

        // Now cache at an earlier block and see that it doesn't override the most
        // recent entry.
        *values.lock().unwrap() = vec![TestValue::new(0, "baz")];
        let result = cache
            .fetch(test_keys(0..1), Block::Number(4))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "baz")]);

        // We still get the cache hit from block 6.
        let result = cache
            .fetch(test_keys(0..1), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![TestValue::new(0, "bar")]);
    }

    #[test]
    fn evicts_old_blocks_from_cache() {
        let fetcher = FakeCacheFetcher::default();
        let block_number = 10u64;
        let block_stream = current_block::mock_single_block(BlockInfo {
            number: block_number,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                number_of_blocks_to_cache: NonZeroU64::new(5).unwrap(),
                number_of_entries_to_auto_update: NonZeroUsize::new(2).unwrap(),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap();

        cache
            .fetch(test_keys(0..10), Block::Number(10))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 10);
        cache
            .update_cache_at_block(14)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 12);
        cache
            .update_cache_at_block(15)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 4);
    }

    #[test]
    fn respects_max_age_limit_for_recent() {
        let fetcher = FakeCacheFetcher::default();
        let block_number = 10u64;
        let block_stream = current_block::mock_single_block(BlockInfo {
            number: block_number,
            ..Default::default()
        });
        let cache = RecentBlockCache::new(
            CacheConfig {
                number_of_blocks_to_cache: NonZeroU64::new(5).unwrap(),
                maximum_recent_block_age: 2,
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap();
        let key = TestKey(0);

        // cache at block 7, most recent block is 10.
        cache
            .fetch(std::iter::once(key), Block::Number(7))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(cache.mutexed.lock().unwrap().get(key, Some(7)).is_some());
        assert!(cache.mutexed.lock().unwrap().get(key, None).is_none());

        // cache at block 8
        cache
            .fetch(std::iter::once(key), Block::Number(8))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(cache.mutexed.lock().unwrap().get(key, Some(7)).is_some());
        assert!(cache.mutexed.lock().unwrap().get(key, Some(8)).is_some());
        assert!(cache.mutexed.lock().unwrap().get(key, None).is_some());
    }
}
