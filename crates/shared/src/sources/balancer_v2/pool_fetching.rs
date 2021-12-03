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
    pools::common::PoolInfoFetcher,
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
    pub common: TokenState,
    pub weight: Bfp,
}

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
        balances: Vec<U256>,
        swap_fee: Bfp,
        paused: bool,
    ) -> Self {
        let mut reserves = HashMap::new();
        // We expect the weight and token indices are aligned with balances returned from EVM query.
        // If necessary we would also pass the tokens along with the query result,
        // use them and fetch the weights from the registry by token address.
        for (&token, balance, &scaling_exponent, &weight) in itertools::izip!(
            &pool_data.common.tokens,
            balances,
            &pool_data.common.scaling_exponents,
            &pool_data.weights
        ) {
            reserves.insert(
                token,
                WeightedTokenState {
                    common: TokenState {
                        balance,
                        scaling_exponent,
                    },
                    weight,
                },
            );
        }
        WeightedPool {
            common: CommonPoolState {
                id: pool_data.common.id,
                address: pool_data.common.address,
                swap_fee,
                paused,
            },
            reserves,
        }
    }
}

impl BalancerPoolEvaluating for WeightedPool {
    fn properties(&self) -> CommonPoolState {
        self.common.clone()
    }
}

pub type AmplificationParameter = super::pools::stable::AmplificationParameter;

#[derive(Clone, Debug)]
pub struct StablePool {
    pub common: CommonPoolState,
    pub reserves: HashMap<H160, TokenState>,
    pub amplification_parameter: AmplificationParameter,
}

impl StablePool {
    pub fn new(
        pool_data: RegisteredStablePool,
        balances: Vec<U256>,
        swap_fee: Bfp,
        amplification_factor: U256,
        amplification_precision: U256,
        paused: bool,
    ) -> Result<Self> {
        let mut reserves = HashMap::new();
        // We expect the weight and token indices are aligned with balances returned from EVM query.
        // If necessary we would also pass the tokens along with the query result,
        // use them and fetch the weights from the registry by token address.
        for (&token, balance, &scaling_exponent) in itertools::izip!(
            &pool_data.common.tokens,
            balances,
            &pool_data.common.scaling_exponents,
        ) {
            reserves.insert(
                token,
                TokenState {
                    balance,
                    scaling_exponent,
                },
            );
        }
        let amplification_parameter =
            AmplificationParameter::new(amplification_factor, amplification_precision)?;
        Ok(StablePool {
            common: CommonPoolState {
                id: pool_data.common.id,
                address: pool_data.common.address,
                swap_fee,
                paused,
            },
            reserves,
            amplification_parameter,
        })
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
        let pool_registry =
            Arc::new(BalancerPoolRegistry::new(web3.clone(), pool_initializer, pool_info).await?);
        let stable_pool_reserve_fetcher =
            PoolReserveFetcher::new(pool_registry.clone(), web3.clone()).await?;
        let weighted_pool_reserve_fetcher =
            PoolReserveFetcher::new(pool_registry.clone(), web3).await?;
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
    use num::BigRational;

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

    #[test]
    fn amplification_parameter_conversions() {
        assert_eq!(
            AmplificationParameter::new(2.into(), 3.into())
                .unwrap()
                .as_u256(),
            6.into()
        );
        assert_eq!(
            AmplificationParameter::new(7.into(), 8.into())
                .unwrap()
                .as_big_rational(),
            BigRational::new(7.into(), 8.into())
        );

        assert_eq!(
            AmplificationParameter::new(1.into(), 0.into())
                .unwrap_err()
                .to_string(),
            "Zero precision not allowed"
        );
    }
}
