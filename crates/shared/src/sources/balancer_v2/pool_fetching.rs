//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `Weighted` and `Stable`
//! pools to be consumed by external users (e.g. Price Estimators and Solvers).

use super::{
    event_handler::BalancerPoolRegistry,
    pool_cache::{
        BalancerPoolCacheMetrics, PoolReserveFetcher, StablePoolReserveCache,
        WeightedPoolReserveCache,
    },
    pool_init::SubgraphPoolInitializer,
    pool_storage::{RegisteredStablePool, RegisteredWeightedPool},
    pools::{
        common::{self, PoolInfoFetcher},
        stable, weighted,
    },
    swap::fixed_point::Bfp,
};
use crate::{
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig, RecentBlockCache},
    token_info::TokenInfoFetching,
    Web3,
};
use anyhow::Result;
use contracts::BalancerV2Vault;
use ethcontract::{H160, H256};
use model::TokenPair;
use reqwest::Client;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub use common::TokenState;
pub use stable::AmplificationParameter;
pub use weighted::TokenState as WeightedTokenState;

pub trait BalancerPoolEvaluating {
    fn properties(&self) -> CommonPoolState;
}

#[derive(Clone, Debug)]
pub struct CommonPoolState {
    pub id: H256,
    pub address: H160,
    pub swap_fee: Bfp,
    pub paused: bool,
}

#[derive(Clone, Debug)]
pub struct WeightedPool {
    pub common: CommonPoolState,
    pub reserves: HashMap<H160, WeightedTokenState>,
}

impl WeightedPool {
    pub fn new(
        pool_data: RegisteredWeightedPool,
        common_state: common::PoolState,
        weighted_state: weighted::PoolState,
    ) -> Self {
        WeightedPool {
            common: CommonPoolState {
                id: pool_data.common.id,
                address: pool_data.common.address,
                swap_fee: common_state.swap_fee,
                paused: common_state.paused,
            },
            reserves: weighted_state.tokens.into_iter().collect(),
        }
    }
}

impl BalancerPoolEvaluating for WeightedPool {
    fn properties(&self) -> CommonPoolState {
        self.common.clone()
    }
}

#[derive(Clone, Debug)]
pub struct StablePool {
    pub common: CommonPoolState,
    pub reserves: HashMap<H160, TokenState>,
    pub amplification_parameter: AmplificationParameter,
}

impl StablePool {
    pub fn new(
        pool_data: RegisteredStablePool,
        common_state: common::PoolState,
        stable_state: stable::PoolState,
    ) -> Self {
        StablePool {
            common: CommonPoolState {
                id: pool_data.common.id,
                address: pool_data.common.address,
                swap_fee: common_state.swap_fee,
                paused: common_state.paused,
            },
            reserves: stable_state.tokens.into_iter().collect(),
            amplification_parameter: stable_state.amplification_parameter,
        }
    }
}

impl BalancerPoolEvaluating for StablePool {
    fn properties(&self) -> CommonPoolState {
        self.common.clone()
    }
}

pub struct FetchedBalancerPools {
    pub stable_pools: Vec<StablePool>,
    pub weighted_pools: Vec<WeightedPool>,
}

impl FetchedBalancerPools {
    pub fn relevant_tokens(&self) -> HashSet<H160> {
        let mut tokens = HashSet::new();
        tokens.extend(
            self.stable_pools
                .iter()
                .flat_map(|pool| pool.reserves.keys().copied()),
        );
        tokens.extend(
            self.weighted_pools
                .iter()
                .flat_map(|pool| pool.reserves.keys().copied()),
        );
        tokens
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait BalancerPoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<FetchedBalancerPools>;
}

pub struct BalancerPoolFetcher {
    pool_registry: Arc<BalancerPoolRegistry>,
    stable_pool_reserve_cache: StablePoolReserveCache,
    weighted_pool_reserve_cache: WeightedPoolReserveCache,
}

impl BalancerPoolFetcher {
    pub async fn new(
        chain_id: u64,
        web3: Web3,
        token_info_fetcher: Arc<dyn TokenInfoFetching>,
        config: CacheConfig,
        block_stream: CurrentBlockStream,
        metrics: Arc<dyn BalancerPoolCacheMetrics>,
        client: Client,
    ) -> Result<Self> {
        let pool_info = Arc::new(PoolInfoFetcher::new(
            BalancerV2Vault::deployed(&web3).await?,
            token_info_fetcher,
        ));
        let pool_initializer = SubgraphPoolInitializer::new(chain_id, client)?;
        let pool_registry = Arc::new(
            BalancerPoolRegistry::new(web3.clone(), pool_initializer, pool_info.clone()).await?,
        );
        let stable_pool_reserve_fetcher =
            PoolReserveFetcher::new(pool_registry.clone(), pool_info.clone(), web3.clone()).await?;
        let weighted_pool_reserve_fetcher =
            PoolReserveFetcher::new(pool_registry.clone(), pool_info, web3).await?;
        let stable_pool_reserve_cache = RecentBlockCache::new(
            config,
            stable_pool_reserve_fetcher,
            block_stream.clone(),
            metrics.clone(),
        )?;
        let weighted_pool_reserve_cache =
            RecentBlockCache::new(config, weighted_pool_reserve_fetcher, block_stream, metrics)?;
        Ok(Self {
            pool_registry,
            stable_pool_reserve_cache,
            weighted_pool_reserve_cache,
        })
    }
}

#[async_trait::async_trait]
impl BalancerPoolFetching for BalancerPoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<FetchedBalancerPools> {
        let pool_ids = self
            .pool_registry
            .pool_ids_for_token_pairs(&token_pairs)
            .await;
        let fetched_stable_pools = self
            .stable_pool_reserve_cache
            .fetch(pool_ids.clone(), at_block)
            .await?;
        let fetched_weighted_pools = self
            .weighted_pool_reserve_cache
            .fetch(pool_ids, at_block)
            .await?;
        // Return only those pools which are not paused.
        Ok(FetchedBalancerPools {
            stable_pools: filter_paused(fetched_stable_pools),
            weighted_pools: filter_paused(fetched_weighted_pools),
        })
    }
}

fn filter_paused<T: BalancerPoolEvaluating>(pools: Vec<T>) -> Vec<T> {
    pools
        .into_iter()
        .filter(|pool| !pool.properties().paused)
        .collect()
}

#[async_trait::async_trait]
impl Maintaining for BalancerPoolFetcher {
    async fn run_maintenance(&self) -> Result<()> {
        futures::try_join!(
            self.pool_registry.run_maintenance(),
            self.stable_pool_reserve_cache.update_cache(),
            self.weighted_pool_reserve_cache.update_cache(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_paused_pools() {
        let pools = vec![
            WeightedPool {
                common: CommonPoolState {
                    id: H256::from_low_u64_be(0),
                    address: Default::default(),
                    swap_fee: Bfp::zero(),
                    paused: true,
                },
                reserves: Default::default(),
            },
            WeightedPool {
                common: CommonPoolState {
                    id: H256::from_low_u64_be(1),
                    address: Default::default(),
                    swap_fee: Bfp::zero(),
                    paused: false,
                },
                reserves: Default::default(),
            },
        ];

        let filtered_pools = filter_paused(pools.clone());
        assert_eq!(filtered_pools.len(), 1);
        assert_eq!(filtered_pools[0].common.id, pools[1].common.id);
    }
}
