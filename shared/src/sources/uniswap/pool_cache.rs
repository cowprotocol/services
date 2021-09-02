use crate::{
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{
        Block, CacheConfig, CacheFetching, CacheKey, CacheMetrics, RecentBlockCache,
    },
    sources::uniswap::pool_fetching::{Pool, PoolFetching},
};
use anyhow::Result;
use model::TokenPair;
use std::collections::HashSet;

pub struct PoolCache(
    RecentBlockCache<TokenPair, Pool, Box<dyn PoolFetching>, &'static PoolCacheMetrics>,
);

impl CacheKey<Pool> for TokenPair {
    fn first_ord() -> Self {
        TokenPair::first_ord()
    }

    fn for_value(value: &Pool) -> Self {
        value.tokens
    }
}

#[async_trait::async_trait]
impl CacheFetching<TokenPair, Pool> for Box<dyn PoolFetching> {
    async fn fetch_values(&self, keys: HashSet<TokenPair>, block: Block) -> Result<Vec<Pool>> {
        self.fetch(keys, block).await
    }
}

impl PoolCache {
    /// Creates a new pool cache.
    pub fn new(
        config: CacheConfig,
        fetcher: Box<dyn PoolFetching>,
        block_stream: CurrentBlockStream,
    ) -> Result<Self> {
        Ok(Self(RecentBlockCache::new(
            config,
            fetcher,
            block_stream,
            PoolCacheMetrics::instance(),
        )?))
    }
}

#[async_trait::async_trait]
impl PoolFetching for PoolCache {
    async fn fetch(&self, pairs: HashSet<TokenPair>, block: Block) -> Result<Vec<Pool>> {
        self.0.fetch(pairs, block).await
    }
}

#[async_trait::async_trait]
impl Maintaining for PoolCache {
    async fn run_maintenance(&self) -> Result<()> {
        self.0.update_cache().await
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "pool_cache")]
struct PoolCacheMetrics {
    /// Number of cache hits in the pool fetcher cache.
    hits: prometheus::IntCounter,

    /// Number of cache misses in the pool fetcher cache.
    misses: prometheus::IntCounter,
}

impl PoolCacheMetrics {
    fn instance() -> &'static Self {
        lazy_static::lazy_static! {
            static ref INSTANCE: PoolCacheMetrics =
                PoolCacheMetrics::new(crate::metrics::get_metrics_registry()).unwrap();
        }

        &INSTANCE
    }
}

impl CacheMetrics for &PoolCacheMetrics {
    fn entries_fetched(&self, cache_hits: usize, cache_misses: usize) {
        self.hits.inc_by(cache_hits as _);
        self.misses.inc_by(cache_misses as _);
    }
}
