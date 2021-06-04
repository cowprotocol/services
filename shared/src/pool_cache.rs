use crate::{
    current_block::{self, CurrentBlockStream},
    maintenance::Maintaining,
    pool_fetching::{Block, Pool, PoolFetching},
};
use anyhow::{Context, Result};
use lru::LruCache;
use model::TokenPair;
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap, HashSet},
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

/// Caching pool fetcher
///
/// Caches all requests for a specific number of blocks and automatically updates the N most
/// recently used pools automatically when a new block arrives.
pub struct PoolCache {
    mutexed: Mutex<Mutexed>,
    number_of_blocks_to_cache: NonZeroU64,
    inner: Box<dyn PoolFetching>,
    block_stream: CurrentBlockStream,
    metrics: Arc<dyn PoolCacheMetrics>,
}

pub trait PoolCacheMetrics: Send + Sync {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize);
}

pub struct NoopPoolCacheMetrics;
impl PoolCacheMetrics for NoopPoolCacheMetrics {
    fn pools_fetched(&self, _: usize, _: usize) {}
}

// Design:
// The design of this module is driven by the need to always return pools quickly so that end users
// going through the api do not have to wait longer than necessary:
// - The mutex is never locked while waiting on an async operation (getting pools from the node).
// - Automatically updating the cache is decoupled from normal pool fetches.
// A result of this is that it is possible that the same uncached pair is requested multiple times
// simultaneously and some work is wasted. This is unlikely to happen in practice and the pool is
// going to be cached the next time it is needed.
// When pools are requested we mark all those pools as recently used which potentially evicts other
// pairs from the pair lru cache. Cache misses are fetched and inserted into the cache.
// Then when the automatic update runs the next time, we request and cache all recently used pairs.
// For some consumers we only care about the "recent" state of the pools. So we can return any
// result from the cache even if it comes from previous blocks.
// On the other hand for others we need to get the pool at exact blocks which is why we keep a cache
// of previous blocks in the first place as we could simplify this module if it was only used by
// by the former.

impl PoolCache {
    /// number_of_blocks_to_cache: Previous blocks stay cached until the block is this much older
    /// than the current block. If there is a request for a block that is already too old then the
    /// result stays cached until the automatic updating runs the next time.
    ///
    /// number_of_pairs_to_auto_update: The number of most recently used pools to keep track of and
    /// auto update when the current block changes.
    ///
    /// maximum_recent_block_age: When a recent block is requested, this is the maximum a cached
    /// block can have to be considered.
    pub fn new(
        number_of_blocks_to_cache: NonZeroU64,
        number_of_pairs_to_auto_update: usize,
        maximum_recent_block_age: u64,
        inner: Box<dyn PoolFetching>,
        block_stream: CurrentBlockStream,
        metrics: Arc<dyn PoolCacheMetrics>,
    ) -> Result<Self> {
        let block = current_block::block_number(&block_stream.borrow())?;
        Ok(Self {
            mutexed: Mutex::new(Mutexed::new(
                number_of_pairs_to_auto_update,
                block,
                maximum_recent_block_age,
            )),
            number_of_blocks_to_cache,
            inner,
            block_stream,
            metrics,
        })
    }

    pub async fn update_cache(&self, new_block: u64) -> Result<()> {
        let pairs = self
            .mutexed
            .lock()
            .unwrap()
            .recently_used_pairs()
            .collect::<HashSet<_>>();
        tracing::debug!("automatically updating {} pair pools", pairs.len());
        let pools = self
            .inner
            .fetch(pairs.clone(), Block::Number(new_block))
            .await?;
        {
            let mut mutexed = self.mutexed.lock().unwrap();
            mutexed.insert(new_block, pairs.into_iter(), &pools);
            let oldest_to_keep = new_block.saturating_sub(self.number_of_blocks_to_cache.get() - 1);
            mutexed.remove_cached_blocks_older_than(oldest_to_keep);
            mutexed.last_update_block = new_block;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl PoolFetching for PoolCache {
    async fn fetch(&self, pairs: HashSet<TokenPair>, block: Block) -> Result<Vec<Pool>> {
        let block = match block {
            Block::Recent => None,
            Block::Number(number) => Some(number),
        };

        let mut cache_hits = Vec::new();
        let mut cache_misses = HashSet::new();
        let last_update_block;
        {
            let mut mutexed = self.mutexed.lock().unwrap();
            for &pair in &pairs {
                match mutexed.get(pair, block) {
                    Some(pools) => cache_hits.extend_from_slice(&pools),
                    None => {
                        cache_misses.insert(pair);
                    }
                }
            }
            last_update_block = mutexed.last_update_block;
        }

        self.metrics
            .pools_fetched(pairs.len() - cache_misses.len(), cache_misses.len());

        if cache_misses.is_empty() {
            return Ok(cache_hits);
        }

        let cache_miss_block = block.unwrap_or(last_update_block);
        let uncached_pools = self
            .inner
            .fetch(cache_misses.clone(), Block::Number(cache_miss_block))
            .await?;
        {
            let mut mutexed = self.mutexed.lock().unwrap();
            mutexed.insert(cache_miss_block, cache_misses.into_iter(), &uncached_pools);
        }

        cache_hits.extend_from_slice(&uncached_pools);
        Ok(cache_hits)
    }
}

#[async_trait::async_trait]
impl Maintaining for PoolCache {
    async fn run_maintenance(&self) -> Result<()> {
        let block = current_block::block_number(&self.block_stream.borrow())?;
        self.update_cache(block)
            .await
            .context("failed to update pool cache")
    }
}

#[derive(Debug)]
struct Mutexed {
    recently_used: LruCache<TokenPair, ()>,
    // For quickly finding at which block a pair is cached.
    cached_most_recently_at_block: HashMap<TokenPair, u64>,
    // Tuple ordering allows us to efficiently construct range queries by block.
    pools: BTreeMap<(u64, TokenPair), Vec<Pool>>,
    // The last block at which the automatic cache updating happened.
    last_update_block: u64,
    // Maximum age a cached block can have to count as recent.
    maximum_recent_block_age: u64,
}

impl Mutexed {
    fn new(pairs_lru_size: usize, current_block: u64, maximum_recent_block_age: u64) -> Mutexed {
        Self {
            recently_used: LruCache::new(pairs_lru_size),
            cached_most_recently_at_block: HashMap::new(),
            pools: BTreeMap::new(),
            last_update_block: current_block,
            maximum_recent_block_age,
        }
    }
}

impl Mutexed {
    fn get(&mut self, pair: TokenPair, block: Option<u64>) -> Option<&[Pool]> {
        self.recently_used.put(pair, ());
        let block = block.or_else(|| {
            self.cached_most_recently_at_block
                .get(&pair)
                .copied()
                .filter(|&block| {
                    self.last_update_block.saturating_sub(block) <= self.maximum_recent_block_age
                })
        })?;
        self.pools.get(&(block, pair)).map(Vec::as_slice)
    }

    fn insert(&mut self, block: u64, pairs: impl Iterator<Item = TokenPair>, pools: &[Pool]) {
        for pair in pairs {
            match self.cached_most_recently_at_block.entry(pair) {
                Entry::Occupied(mut entry) => {
                    let value = entry.get_mut();
                    *value = (*value).max(block);
                }
                Entry::Vacant(entry) => {
                    entry.insert(block);
                }
            }
            // Make sure pairs without pools are cached.
            self.pools.insert((block, pair), Vec::new());
        }
        for &pool in pools {
            // Unwrap because previous loop guarantees all pairs have an entry.
            self.pools
                .get_mut(&(block, pool.tokens))
                .unwrap()
                .push(pool);
        }
    }

    fn remove_cached_blocks_older_than(&mut self, oldest_to_keep: u64) {
        tracing::debug!("dropping blocks older than {} from cache", oldest_to_keep);
        self.pools = self
            .pools
            .split_off(&(oldest_to_keep, TokenPair::first_ord()));
        self.cached_most_recently_at_block
            .retain(|_pair, block| *block >= oldest_to_keep);
        tracing::debug!(
            "the cache now contains pools for {} block-pair combinations",
            self.pools.len()
        );
    }

    fn recently_used_pairs(&self) -> impl Iterator<Item = TokenPair> + '_ {
        self.recently_used.iter().map(|(pair, _)| *pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::current_block::Block as Web3Block;
    use futures::FutureExt;
    use primitive_types::H160;
    use std::sync::Arc;
    use tokio::sync::watch;

    #[derive(Default)]
    struct FakePoolFetcher(Arc<Mutex<Vec<Pool>>>);
    #[async_trait::async_trait]
    impl PoolFetching for FakePoolFetcher {
        async fn fetch(&self, _: HashSet<TokenPair>, _: Block) -> Result<Vec<Pool>> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    fn test_pairs() -> [TokenPair; 3] {
        [
            TokenPair::new(H160::from_low_u64_le(0), H160::from_low_u64_le(1)).unwrap(),
            TokenPair::new(H160::from_low_u64_le(1), H160::from_low_u64_le(2)).unwrap(),
            TokenPair::new(H160::from_low_u64_le(2), H160::from_low_u64_le(3)).unwrap(),
        ]
    }

    #[test]
    fn marks_recently_used() {
        let inner = FakePoolFetcher::default();
        let block_number = 10u64;
        let block = Web3Block {
            number: Some(block_number.into()),
            ..Default::default()
        };
        let (_sender, receiver) = watch::channel(block);
        let cache = PoolCache::new(
            NonZeroU64::new(1).unwrap(),
            2,
            10,
            Box::new(inner),
            receiver,
            Arc::new(NoopPoolCacheMetrics),
        )
        .unwrap();

        cache
            .fetch(test_pairs()[0..1].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        cache
            .fetch(test_pairs()[1..2].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        let pairs = cache
            .mutexed
            .lock()
            .unwrap()
            .recently_used_pairs()
            .collect::<HashSet<_>>();
        assert_eq!(pairs, test_pairs()[0..2].iter().copied().collect());

        // 1 is already cached, 3 isn't.
        cache
            .fetch(test_pairs()[1..3].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        let pairs = cache
            .mutexed
            .lock()
            .unwrap()
            .recently_used_pairs()
            .collect::<HashSet<_>>();
        assert_eq!(pairs, test_pairs()[1..3].iter().copied().collect());
    }

    #[test]
    fn auto_updates_recently_used() {
        let inner = FakePoolFetcher::default();
        let pools = inner.0.clone();
        let block_number = 10u64;
        let block = Web3Block {
            number: Some(block_number.into()),
            ..Default::default()
        };
        let (_sender, receiver) = watch::channel(block);
        let cache = PoolCache::new(
            NonZeroU64::new(1).unwrap(),
            2,
            10,
            Box::new(inner),
            receiver,
            Arc::new(NoopPoolCacheMetrics),
        )
        .unwrap();

        let result = cache
            .fetch(test_pairs()[0..2].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(result.is_empty());

        let updated_pools = vec![
            Pool::uniswap(test_pairs()[0], (1, 1)),
            Pool::uniswap(test_pairs()[1], (2, 2)),
        ];
        *pools.lock().unwrap() = updated_pools.clone();
        cache
            .update_cache(block_number)
            .now_or_never()
            .unwrap()
            .unwrap();
        pools.lock().unwrap().clear();

        let result = cache
            .fetch(test_pairs()[0..2].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 2);
        for pool in updated_pools {
            assert!(result.contains(&pool));
        }
    }

    #[test]
    fn cache_hit_and_miss() {
        let inner = FakePoolFetcher::default();
        let pools = inner.0.clone();
        let block_number = 10u64;
        let block = Web3Block {
            number: Some(block_number.into()),
            ..Default::default()
        };
        let (_sender, receiver) = watch::channel(block);
        let cache = PoolCache::new(
            NonZeroU64::new(1).unwrap(),
            2,
            10,
            Box::new(inner),
            receiver,
            Arc::new(NoopPoolCacheMetrics),
        )
        .unwrap();

        let pool0 = Pool::uniswap(test_pairs()[0], (0, 0));
        let pool1 = Pool::uniswap(test_pairs()[1], (1, 1));
        let pool2 = Pool::uniswap(test_pairs()[2], (2, 2));

        *pools.lock().unwrap() = vec![pool0, pool1];
        // cache miss gets cached
        cache
            .fetch(test_pairs()[0..2].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();

        *pools.lock().unwrap() = vec![pool2];
        // pair 1 is cache hit, pair 2 is miss
        let result = cache
            .fetch(test_pairs()[1..3].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&pool1));
        assert!(result.contains(&pool2));

        // Make sure everything is still properly cached.
        pools.lock().unwrap().clear();
        let result = cache
            .fetch(test_pairs()[0..3].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&pool0));
        assert!(result.contains(&pool1));
        assert!(result.contains(&pool2));
    }

    #[test]
    fn uses_most_recent_cached_for_latest_block() {
        let inner = FakePoolFetcher::default();
        let pools = inner.0.clone();
        let block_number = 10u64;
        let block = Web3Block {
            number: Some(block_number.into()),
            ..Default::default()
        };
        let (_sender, receiver) = watch::channel(block);
        let cache = PoolCache::new(
            NonZeroU64::new(1).unwrap(),
            2,
            10,
            Box::new(inner),
            receiver,
            Arc::new(NoopPoolCacheMetrics),
        )
        .unwrap();

        // cache at block 5
        *pools.lock().unwrap() = vec![Pool::uniswap(test_pairs()[0], (1, 1))];
        let result = cache
            .fetch(
                test_pairs()[0..1].iter().copied().collect(),
                Block::Number(5),
            )
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![Pool::uniswap(test_pairs()[0], (1, 1))]);

        // cache at block 6
        *pools.lock().unwrap() = vec![Pool::uniswap(test_pairs()[0], (2, 2))];
        let result = cache
            .fetch(
                test_pairs()[0..1].iter().copied().collect(),
                Block::Number(6),
            )
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![Pool::uniswap(test_pairs()[0], (2, 2))]);

        pools.lock().unwrap().clear();
        // cache hit at block 6
        let result = cache
            .fetch(test_pairs()[0..1].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![Pool::uniswap(test_pairs()[0], (2, 2))]);

        // Now cache at an earlier block and see that it doesn't override the most recent entry.
        *pools.lock().unwrap() = vec![Pool::uniswap(test_pairs()[0], (3, 3))];
        let result = cache
            .fetch(
                test_pairs()[0..1].iter().copied().collect(),
                Block::Number(4),
            )
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![Pool::uniswap(test_pairs()[0], (3, 3))]);

        // We still get the cache hit from block 6.
        let result = cache
            .fetch(test_pairs()[0..1].iter().copied().collect(), Block::Recent)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, vec![Pool::uniswap(test_pairs()[0], (2, 2))]);
    }

    #[test]
    fn evicts_old_blocks_from_cache() {
        let inner = FakePoolFetcher::default();
        let block_number = 10u64;
        let block = Web3Block {
            number: Some(block_number.into()),
            ..Default::default()
        };
        let (_sender, receiver) = watch::channel(block);
        let cache = PoolCache::new(
            NonZeroU64::new(5).unwrap(),
            0,
            10,
            Box::new(inner),
            receiver,
            Arc::new(NoopPoolCacheMetrics),
        )
        .unwrap();

        cache
            .fetch(
                test_pairs()[0..1].iter().copied().collect(),
                Block::Number(10),
            )
            .now_or_never()
            .unwrap()
            .unwrap();

        assert_eq!(cache.mutexed.lock().unwrap().pools.len(), 1);
        cache.update_cache(14).now_or_never().unwrap().unwrap();
        assert_eq!(cache.mutexed.lock().unwrap().pools.len(), 1);
        cache.update_cache(15).now_or_never().unwrap().unwrap();
        assert!(cache.mutexed.lock().unwrap().pools.is_empty());
    }

    #[test]
    fn respects_max_age_limit_for_recent() {
        let inner = FakePoolFetcher::default();
        let block_number = 10u64;
        let block = Web3Block {
            number: Some(block_number.into()),
            ..Default::default()
        };
        let (_sender, receiver) = watch::channel(block);
        let cache = PoolCache::new(
            NonZeroU64::new(5).unwrap(),
            0,
            2,
            Box::new(inner),
            receiver,
            Arc::new(NoopPoolCacheMetrics),
        )
        .unwrap();
        let pair = test_pairs()[0];

        // cache at block 7, most recent block is 10.
        cache
            .fetch(std::iter::once(pair).collect(), Block::Number(7))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(cache.mutexed.lock().unwrap().get(pair, Some(7)).is_some());
        assert!(cache.mutexed.lock().unwrap().get(pair, None).is_none());

        // cache at block 8
        cache
            .fetch(std::iter::once(pair).collect(), Block::Number(8))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(cache.mutexed.lock().unwrap().get(pair, Some(7)).is_some());
        assert!(cache.mutexed.lock().unwrap().get(pair, Some(8)).is_some());
        assert!(cache.mutexed.lock().unwrap().get(pair, None).is_some());
    }
}
