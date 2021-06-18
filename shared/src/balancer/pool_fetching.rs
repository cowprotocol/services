//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `WeightedPools`
//! to be consumed by external users (e.g. Price Estimators and Solvers).
use anyhow::Result;
use model::TokenPair;
use std::collections::HashSet;

use crate::balancer::pool_cache::{BalancerPoolReserveCache, PoolCacheMetrics, PoolReserveFetcher};
use crate::balancer::{event_handler::BalancerPoolRegistry, pool_storage::WeightedPool};
use crate::current_block::CurrentBlockStream;
use crate::maintenance::Maintaining;
use crate::recent_block_cache::{Block, CacheConfig, RecentBlockCache};
use crate::token_info::TokenInfoFetching;
use crate::Web3;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait WeightedPoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<WeightedPool>>;
}

pub struct BalancerPoolFetcher {
    pool_registry: Arc<BalancerPoolRegistry>,
    pool_reserve_cache: BalancerPoolReserveCache,
}

impl BalancerPoolFetcher {
    pub async fn new(
        web3: Web3,
        token_info_fetcher: Arc<dyn TokenInfoFetching>,
        config: CacheConfig,
        block_stream: CurrentBlockStream,
        metrics: Arc<dyn PoolCacheMetrics>,
    ) -> Result<Self> {
        let pool_registry =
            Arc::new(BalancerPoolRegistry::new(web3.clone(), token_info_fetcher).await?);
        let reserve_fetcher = PoolReserveFetcher::new(pool_registry.clone(), web3).await?;
        let pool_reserve_cache =
            RecentBlockCache::new(config, reserve_fetcher, block_stream, metrics)?;
        Ok(Self {
            pool_registry,
            pool_reserve_cache,
        })
    }
}

#[async_trait::async_trait]
impl WeightedPoolFetching for BalancerPoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<WeightedPool>> {
        let pool_ids = self
            .pool_registry
            .get_pool_ids_containing_token_pairs(token_pairs)
            .await;
        self.pool_reserve_cache.fetch(pool_ids, at_block).await
    }
}

#[async_trait::async_trait]
impl Maintaining for BalancerPoolFetcher {
    async fn run_maintenance(&self) -> Result<()> {
        futures::try_join!(
            self.pool_registry.run_maintenance(),
            self.pool_reserve_cache.update_cache(),
        )?;
        Ok(())
    }
}
