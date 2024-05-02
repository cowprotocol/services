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
    crate::request_sharing::BoxRequestSharing,
    anyhow::{Context, Result},
    cached::{Cached, SizedCache},
    ethcontract::BlockNumber,
    ethrpc::current_block::CurrentBlockStream,
    futures::{FutureExt, StreamExt},
    itertools::Itertools,
    prometheus::IntCounterVec,
    std::{
        cmp,
        collections::{hash_map::Entry, BTreeMap, HashMap, HashSet},
        hash::Hash,
        num::{NonZeroU64, NonZeroUsize},
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
    inner: Arc<Inner<K, V, F>>,
}

pub struct Inner<K, V, F>
where
    K: CacheKey<V>,
    F: CacheFetching<K, V>,
{
    mutexed: Mutex<Mutexed<K, V>>,
    number_of_blocks_to_cache: NonZeroU64,
    fetcher: Arc<F>,
    maximum_retries: u32,
    delay_between_retries: Duration,
    metrics: &'static Metrics,
    metrics_label: &'static str,
    requests: BoxRequestSharing<(K, Block), Option<Vec<V>>>,
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
    V: Clone + Send + Sync + 'static,
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
        let inner = Arc::new(Inner {
            mutexed: Mutex::new(Mutexed::new(
                config.number_of_entries_to_auto_update,
                block,
                config.maximum_recent_block_age,
            )),
            number_of_blocks_to_cache: config.number_of_blocks_to_cache,
            fetcher: Arc::new(fetcher),
            maximum_retries: config.max_retries,
            delay_between_retries: config.delay_between_retries,
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
            metrics_label,
            requests: BoxRequestSharing::labelled("liquidity_fetching".into()),
        });

        Self::spawn_gc_task(
            Arc::downgrade(&inner),
            block_stream,
            metrics_label.to_string(),
        );

        Ok(Self { inner })
    }

    pub async fn fetch(&self, keys: impl IntoIterator<Item = K>, block: Block) -> Result<Vec<V>> {
        self.inner.fetch(keys, block).await
    }

    fn spawn_gc_task(
        inner: std::sync::Weak<Inner<K, V, F>>,
        block_stream: CurrentBlockStream,
        label: String,
    ) {
        tokio::task::spawn(
            async move {
                let mut stream = ethrpc::current_block::into_stream(block_stream);
                while let Some(block) = stream.next().await {
                    let Some(inner) = inner.upgrade() else {
                        tracing::debug!("cache no longer in use; terminate GC task");
                        break;
                    };
                    if let Err(err) = inner.update_cache_at_block(block.number).await {
                        tracing::warn!(?err, "failed to update cache");
                    }
                }
            }
            .instrument(tracing::info_span!("cache_maintenance", cache = label)),
        );
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
        let oldest_to_keep = new_block.saturating_sub(self.number_of_blocks_to_cache.get() - 1);
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

    // Sometimes nodes requests error when we try to get state from what we think is
    // the current block when the node has been load balanced out to one that
    // hasn't seen the block yet. As a workaround we repeat the request up to N
    // times while sleeping in between.
    async fn fetch_inner(&self, key: K, block: Block) -> Result<Vec<V>> {
        let retries = self.maximum_retries;
        let delay = self.delay_between_retries;
        let fetcher = self.fetcher.clone();
        let fut = self.requests.shared_or_else((key, block), |entry| {
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
        fut.await.context("could not fetch liquidity")
    }

    async fn fetch(&self, keys: impl IntoIterator<Item = K>, block: Block) -> Result<Vec<V>> {
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
        let cache_misses: Vec<_> = cache_misses.into_iter().collect();
        // Splits fetches into chunks because we can get over 1400 requests when the
        // cache is empty which tend to time out if we don't chunk them.
        for chunk in cache_misses.chunks(REQUEST_BATCH_SIZE) {
            let keys = chunk.iter().cloned().collect();
            let fetched = self
                .fetch_inner_many(keys, Block::Number(cache_miss_block))
                .await?;
            let found_keys = fetched.iter().map(K::for_value).unique().collect_vec();
            cache_hits.extend_from_slice(&fetched);

            let mut mutexed = self.mutexed.lock().unwrap();
            mutexed.insert(cache_miss_block, chunk.iter().cloned(), fetched);
            if block.is_some() {
                // Only if a block number was specified the caller actually cared about the most
                // accurate data for these keys. Only in that case we want to be nice and
                // remember the key for future background updates of the cached
                // liquidity.
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
    recently_used: SizedCache<K, ()>,
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
            recently_used: SizedCache::with_size(entries_lru_size.get()),
            cached_most_recently_at_block: HashMap::new(),
            entries: BTreeMap::new(),
            last_update_block: current_block,
            maximum_recent_block_age,
        }
    }

    fn get(&mut self, key: K, block: Option<u64>) -> Option<&[V]> {
        let allow_background_udpates = block.is_some();
        let block = block.or_else(|| {
            self.cached_most_recently_at_block
                .get(&key)
                .copied()
                .filter(|&block| {
                    self.last_update_block.saturating_sub(block) <= self.maximum_recent_block_age
                })
        })?;
        let result = self.entries.get(&(block, key.clone())).map(Vec::as_slice);
        if allow_background_udpates && result.is_some_and(|values| !values.is_empty()) {
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

        // Iterate from newest block to oldest block and only keep the most recent
        // liquidity around to reduce memory consumption.
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
        ethrpc::current_block::{mock_single_block, BlockInfo},
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

        // Don't cache this because we didn't request the liquidity on a specific block.
        cache.fetch(test_keys(1..2), Block::Recent).await.unwrap();
        assert_keys_recently_used(&[0]);

        // Don't cache this because there is no liquidity for this block on-chain.
        cache
            .fetch(test_keys(2..3), Block::Number(block_number))
            .await
            .unwrap();
        assert_keys_recently_used(&[0]);

        // Cache the new key but evict the other key because we have a limited capacity.
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

        let result = cache.fetch(test_keys(0..4), Block::Recent).await.unwrap();
        // We can fetch data for keys with `Recent` but we don't schedule them for auto
        // updates.
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

        let result = cache.fetch(test_keys(0..4), Block::Recent).await.unwrap();
        assert_eq!(result.len(), 4);
        // These keys were scheduled for background updates and show the new value.
        assert!(result.contains(&updated_values[0]));
        assert!(result.contains(&updated_values[1]));
        // These keys were NOT scheduled for background updates and show the old value.
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
                number_of_blocks_to_cache: NonZeroU64::new(2).unwrap(),
                number_of_entries_to_auto_update: NonZeroUsize::new(2).unwrap(),
                ..Default::default()
            },
            fetcher,
            block_stream,
            "",
        )
        .unwrap()
        .inner;

        // Fetch 10 keys on block 10; but we only have capacity to update 2 of those in
        // the background.
        cache
            .fetch(test_keys(0..10), Block::Number(10))
            .await
            .unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 10);

        block_sender.send(block(11)).unwrap();
        // Fetch updated liquidity for 2 of the initial 10 keys
        cache.update_cache_at_block(11).await.unwrap();
        // Fetch 2 new keys which are NOT scheduled for background updates
        cache.fetch(test_keys(10..12), Block::Recent).await.unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 12);

        block_sender.send(block(12)).unwrap();
        // Fetch updated liquidity for 2 of the initial 10 keys
        cache.update_cache_at_block(12).await.unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().entries.len(), 4);

        block_sender.send(block(13)).unwrap();
        // Update 2 blocks in background but now it's time to evict the 2 additional
        // keys we fetched with `Block::Recent` because we are only allowed to
        // keep state that is up to 2 blocks old.
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
                number_of_blocks_to_cache: NonZeroU64::new(5).unwrap(),
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
