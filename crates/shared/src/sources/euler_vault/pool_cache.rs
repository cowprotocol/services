use {
    crate::{
        recent_block_cache::{Block, CacheConfig, CacheFetching, CacheKey, RecentBlockCache},
        sources::{euler_vault::pool_fetching::DepositContract, euler_vault::pool_fetching::PoolFetching},
    }, anyhow::Result, ethcontract::H160, ethrpc::block_stream::CurrentBlockWatcher, model::TokenPair, std::{collections::HashSet, sync::Arc}
};

pub struct DepositContractCache(RecentBlockCache<H160, DepositContract, Arc<dyn PoolFetching>>);

impl CacheKey<DepositContract> for H160 {
    fn first_ord() -> Self {
        H160::first_ord()
    }

    fn for_value(value: &DepositContract) -> Self {
        value.address
    }
}

#[async_trait::async_trait]
impl CacheFetching<H160, DepositContract> for Arc<dyn PoolFetching> {
    async fn fetch_values(&self, keys: HashSet<H160>, block: Block) -> Result<Vec<DepositContract>> {
        self.fetch(keys, block).await
    }
}

impl DepositContractCache {
    /// Creates a new pool cache.
    pub fn new(
        config: CacheConfig,
        fetcher: Arc<dyn PoolFetching>,
        block_stream: CurrentBlockWatcher,
    ) -> Result<Self> {
        Ok(Self(RecentBlockCache::new(
            config,
            fetcher,
            block_stream,
            "euler_vaults",
        )?))
    }
}

#[async_trait::async_trait]
impl PoolFetching for DepositContractCache {
    async fn fetch(&self, pairs: HashSet<TokenPair>, block: Block) -> Result<Vec<DepositContract>> {
        self.0.fetch(pairs, block).await
    }
}
