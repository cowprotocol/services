use crate::{
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig, CacheFetching, CacheKey, RecentBlockCache},
    sources::uniswap_v2::pool_fetching::{Pool, PoolFetching},
};
use anyhow::Result;
use model::TokenPair;
use std::{collections::HashSet, sync::Arc};

pub struct PoolCache(RecentBlockCache<TokenPair, Pool, Arc<dyn PoolFetching>>);

impl CacheKey<Pool> for TokenPair {
    fn first_ord() -> Self {
        TokenPair::first_ord()
    }

    fn for_value(value: &Pool) -> Self {
        value.tokens
    }
}

#[async_trait::async_trait]
impl CacheFetching<TokenPair, Pool> for Arc<dyn PoolFetching> {
    async fn fetch_values(&self, keys: HashSet<TokenPair>, block: Block) -> Result<Vec<Pool>> {
        self.fetch(keys, block).await
    }
}

impl PoolCache {
    /// Creates a new pool cache.
    pub fn new(
        config: CacheConfig,
        fetcher: Arc<dyn PoolFetching>,
        block_stream: CurrentBlockStream,
    ) -> Result<Self> {
        Ok(Self(RecentBlockCache::new(
            config,
            fetcher,
            block_stream,
            "uniswapv2",
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
