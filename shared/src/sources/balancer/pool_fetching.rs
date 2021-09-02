//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `WeightedPools`
//! to be consumed by external users (e.g. Price Estimators and Solvers).
use crate::sources::balancer::pool_cache::new_balancer_pool_reserve_cache;
use crate::{
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig},
    sources::balancer::{
        event_handler::BalancerPoolRegistry,
        info_fetching::PoolInfoFetcher,
        pool_cache::{BalancerPoolReserveCache, PoolReserveFetcher},
        pool_init::DefaultPoolInitializer,
        pool_storage::RegisteredWeightedPool,
        swap::fixed_point::Bfp,
    },
    token_info::TokenInfoFetching,
    Web3,
};
use anyhow::Result;
use contracts::BalancerV2Vault;
use ethcontract::{H160, H256, U256};
use model::TokenPair;
use reqwest::Client;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenState {
    pub balance: U256,
    pub scaling_exponent: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedTokenState {
    pub token_state: TokenState,
    pub weight: Bfp,
}

#[derive(Clone, Debug)]
pub struct WeightedPool {
    pub pool_id: H256,
    pub pool_address: H160,
    pub swap_fee_percentage: Bfp,
    pub reserves: HashMap<H160, WeightedTokenState>,
    pub paused: bool,
}

impl WeightedPool {
    pub fn new(
        pool_data: RegisteredWeightedPool,
        balances: Vec<U256>,
        swap_fee_percentage: Bfp,
        paused: bool,
    ) -> Self {
        let mut reserves = HashMap::new();
        // We expect the weight and token indices are aligned with balances returned from EVM query.
        // If necessary we would also pass the tokens along with the query result,
        // use them and fetch the weights from the registry by token address.
        for (i, balance) in balances.into_iter().enumerate() {
            reserves.insert(
                pool_data.common.tokens[i],
                WeightedTokenState {
                    token_state: TokenState {
                        balance,
                        scaling_exponent: pool_data.common.scaling_exponents[i],
                    },
                    weight: pool_data.normalized_weights[i],
                },
            );
        }
        WeightedPool {
            pool_id: pool_data.common.pool_id,
            pool_address: pool_data.common.pool_address,
            swap_fee_percentage,
            reserves,
            paused,
        }
    }
}

#[mockall::automock]
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
        chain_id: u64,
        web3: Web3,
        token_info_fetcher: Arc<dyn TokenInfoFetching>,
        config: CacheConfig,
        block_stream: CurrentBlockStream,
        client: Client,
    ) -> Result<Self> {
        let pool_info = Arc::new(PoolInfoFetcher {
            web3: web3.clone(),
            token_info_fetcher: token_info_fetcher.clone(),
            vault: BalancerV2Vault::deployed(&web3).await?,
        });
        let pool_initializer = DefaultPoolInitializer::new(chain_id, pool_info.clone(), client)?;
        let pool_registry =
            Arc::new(BalancerPoolRegistry::new(web3.clone(), pool_initializer, pool_info).await?);
        let reserve_fetcher = PoolReserveFetcher::new(pool_registry.clone(), web3).await?;
        let pool_reserve_cache =
            new_balancer_pool_reserve_cache(config, reserve_fetcher, block_stream)?;
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
        let fetched_pools = self.pool_reserve_cache.fetch(pool_ids, at_block).await?;
        // Return only those pools which are not paused.
        Ok(filter_paused(fetched_pools))
    }
}

fn filter_paused(weighted_pools: Vec<WeightedPool>) -> Vec<WeightedPool> {
    weighted_pools
        .into_iter()
        .filter(|pool| !pool.paused)
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_paused_pools() {
        let pools = vec![
            WeightedPool {
                pool_id: H256::from_low_u64_be(0),
                pool_address: Default::default(),
                swap_fee_percentage: Bfp::zero(),
                reserves: Default::default(),
                paused: true,
            },
            WeightedPool {
                pool_id: H256::from_low_u64_be(1),
                pool_address: Default::default(),
                swap_fee_percentage: Bfp::zero(),
                reserves: Default::default(),
                paused: false,
            },
        ];

        let filtered_pools = filter_paused(pools.clone());
        assert_eq!(filtered_pools.len(), 1);
        assert_eq!(filtered_pools[0].pool_id, pools[1].pool_id);
    }
}
