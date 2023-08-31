//! Module for implementing `RecentBlockCache` interface around an
//! `InnerPoolFetching` implementation.
//!
//! This allows us to turn cache a pool registry.

use {
    super::internal::InternalPoolFetching,
    crate::{
        maintenance::Maintaining,
        recent_block_cache::{Block, CacheConfig, CacheFetching, CacheKey, RecentBlockCache},
        sources::balancer_v2::pools::Pool,
    },
    anyhow::Result,
    ethcontract::H256,
    ethrpc::current_block::CurrentBlockStream,
    std::{collections::HashSet, sync::Arc},
};

/// Internal type alias used for inner recent block cache.
type PoolCache<Inner> = RecentBlockCache<H256, Pool, CacheFetcher<Inner>>;

/// A cached pool fetcher that wraps an inner `InternalPoolFetching`
/// implementation.
pub struct Cache<Inner>
where
    Inner: InternalPoolFetching,
{
    inner: Arc<Inner>,
    cache: PoolCache<Inner>,
}

impl<Inner> Cache<Inner>
where
    Inner: InternalPoolFetching,
{
    pub fn new(
        inner: Inner,
        config: CacheConfig,
        block_stream: CurrentBlockStream,
    ) -> Result<Self> {
        let inner = Arc::new(inner);
        let fetcher = CacheFetcher(inner.clone());
        let cache = RecentBlockCache::new(config, fetcher, block_stream, "balancerv2")?;
        Ok(Self { inner, cache })
    }
}

#[async_trait::async_trait]
impl<Inner> InternalPoolFetching for Cache<Inner>
where
    Inner: InternalPoolFetching,
{
    async fn pool_ids_for_token_pairs(
        &self,
        token_pairs: HashSet<model::TokenPair>,
    ) -> HashSet<H256> {
        self.inner.pool_ids_for_token_pairs(token_pairs).await
    }

    async fn pools_by_id(&self, pool_ids: HashSet<H256>, block: Block) -> Result<Vec<Pool>> {
        self.cache.fetch(pool_ids, block).await
    }
}

#[async_trait::async_trait]
impl<Inner> Maintaining for Cache<Inner>
where
    Inner: InternalPoolFetching,
{
    async fn run_maintenance(&self) -> Result<()> {
        futures::try_join!(self.inner.run_maintenance(), self.cache.update_cache())?;
        Ok(())
    }

    fn name(&self) -> &str {
        "BalancerPoolFetcher"
    }
}

impl CacheKey<Pool> for H256 {
    fn first_ord() -> Self {
        H256::zero()
    }

    fn for_value(pool: &Pool) -> Self {
        pool.id
    }
}

/// Internal struct for implementing `CacheFetching` for `InnerPoolFetching`
/// types.
///
/// This additional new-type is not strictly needed, but avoids leaking cache
/// implementation details.
struct CacheFetcher<Inner>(Arc<Inner>);

#[async_trait::async_trait]
impl<Inner> CacheFetching<H256, Pool> for CacheFetcher<Inner>
where
    Inner: InternalPoolFetching,
{
    async fn fetch_values(&self, pool_ids: HashSet<H256>, at_block: Block) -> Result<Vec<Pool>> {
        self.0.pools_by_id(pool_ids, at_block).await
    }
}
