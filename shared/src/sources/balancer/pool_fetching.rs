//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `WeightedPools`
//! to be consumed by external users (e.g. Price Estimators and Solvers).
use crate::{
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig, RecentBlockCache},
    sources::balancer::{
        event_handler::BalancerPoolRegistry,
        pool_cache::{BalancerPoolReserveCache, PoolReserveFetcher, WeightedPoolCacheMetrics},
        pool_init::DefaultPoolInitializer,
        pool_storage::RegisteredWeightedPool,
        swap::fixed_point::Bfp,
    },
    token_info::TokenInfoFetching,
    Web3,
};
use anyhow::Result;
use ethcontract::{H160, H256, U256};
use model::TokenPair;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolTokenState {
    pub balance: U256,
    pub weight: Bfp,
    pub scaling_exponent: u8,
}

#[derive(Clone, Debug)]
pub struct WeightedPool {
    pub pool_id: H256,
    pub pool_address: H160,
    pub swap_fee_percentage: Bfp,
    pub reserves: HashMap<H160, PoolTokenState>,
}

impl WeightedPool {
    pub fn new(
        pool_data: RegisteredWeightedPool,
        balances: Vec<U256>,
        swap_fee_percentage: Bfp,
    ) -> Self {
        let mut reserves = HashMap::new();
        // We expect the weight and token indices are aligned with balances returned from EVM query.
        // If necessary we would also pass the tokens along with the query result,
        // use them and fetch the weights from the registry by token address.
        for (i, balance) in balances.into_iter().enumerate() {
            reserves.insert(
                pool_data.tokens[i],
                PoolTokenState {
                    balance,
                    weight: pool_data.normalized_weights[i],
                    scaling_exponent: pool_data.scaling_exponents[i],
                },
            );
        }
        WeightedPool {
            pool_id: pool_data.pool_id,
            pool_address: pool_data.pool_address,
            swap_fee_percentage,
            reserves,
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
        metrics: Arc<dyn WeightedPoolCacheMetrics>,
    ) -> Result<Self> {
        let pool_initializer = DefaultPoolInitializer::new(chain_id)?;
        let pool_registry = Arc::new(
            BalancerPoolRegistry::new(web3.clone(), pool_initializer, token_info_fetcher).await?,
        );
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
