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
        info_fetching::PoolInfoFetcher,
        pool_cache::{BalancerPoolCacheMetrics, BalancerPoolReserveCache, PoolReserveFetcher},
        pool_init::DefaultPoolInitializer,
        pool_storage::{PoolType, RegisteredStablePool, RegisteredWeightedPool},
        swap::fixed_point::Bfp,
    },
    token_info::TokenInfoFetching,
    Web3,
};
use anyhow::{anyhow, Result};
use contracts::BalancerV2Vault;
use ethcontract::{H160, H256, U256};
use model::TokenPair;
use num::BigRational;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BalancerPoolState {
    Weighted(WeightedTokenState),
    Stable(TokenState),
}

impl BalancerPoolState {
    pub fn balance(&self) -> U256 {
        match self {
            BalancerPoolState::Weighted(pool) => pool.token_state.balance,
            BalancerPoolState::Stable(pool) => pool.balance,
        }
    }

    pub fn scaling_exponent(&self) -> u8 {
        match self {
            BalancerPoolState::Weighted(pool) => pool.token_state.scaling_exponent,
            BalancerPoolState::Stable(pool) => pool.scaling_exponent,
        }
    }
}

#[derive(Clone, Debug)]
pub enum BalancerPool {
    Weighted(WeightedPool),
    Stable(StablePool),
}

impl BalancerPool {
    pub fn pool_id(&self) -> H256 {
        match self {
            BalancerPool::Weighted(pool) => pool.pool_id,
            BalancerPool::Stable(pool) => pool.pool_id,
        }
    }

    pub fn paused(&self) -> bool {
        match self {
            BalancerPool::Weighted(pool) => pool.paused,
            BalancerPool::Stable(pool) => pool.paused,
        }
    }

    pub fn reserve_keys(&self) -> HashSet<H160> {
        match self {
            BalancerPool::Weighted(pool) => pool.reserves.keys().copied().collect(),
            BalancerPool::Stable(pool) => pool.reserves.keys().copied().collect(),
        }
    }

    pub fn swap_fee_percentage(&self) -> Bfp {
        match self {
            BalancerPool::Weighted(pool) => pool.swap_fee_percentage,
            BalancerPool::Stable(pool) => pool.swap_fee_percentage,
        }
    }

    pub fn pool_type(&self) -> PoolType {
        match self {
            BalancerPool::Weighted(_) => PoolType::Weighted,
            BalancerPool::Stable(_) => PoolType::Stable,
        }
    }

    pub fn reserves(&self) -> HashMap<H160, BalancerPoolState> {
        match self {
            BalancerPool::Weighted(pool) => pool
                .clone()
                .reserves
                .into_iter()
                .map(|(k, v)| (k, BalancerPoolState::Weighted(v)))
                .collect(),
            BalancerPool::Stable(pool) => pool
                .clone()
                .reserves
                .into_iter()
                .map(|(k, v)| (k, BalancerPoolState::Stable(v)))
                .collect(),
        }
    }

    pub fn try_into_weighted(&self) -> Result<WeightedPool> {
        if let BalancerPool::Weighted(pool) = self {
            Ok(pool.clone())
        } else {
            Err(anyhow!("Not a weighted pool!"))
        }
    }

    pub fn try_into_stable(&self) -> Result<StablePool> {
        if let BalancerPool::Stable(pool) = self {
            Ok(pool.clone())
        } else {
            Err(anyhow!("Not a weighted pool!"))
        }
    }

    pub fn is_weighted(&self) -> bool {
        match self {
            BalancerPool::Weighted(_) => true,
            BalancerPool::Stable(_) => false,
        }
    }
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
        for (&token, balance, &scaling_exponent, &weight) in itertools::izip!(
            &pool_data.common.tokens,
            balances,
            &pool_data.common.scaling_exponents,
            &pool_data.normalized_weights
        ) {
            reserves.insert(
                token,
                WeightedTokenState {
                    token_state: TokenState {
                        balance,
                        scaling_exponent,
                    },
                    weight,
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

#[derive(Clone, Debug)]
pub struct StablePool {
    pub pool_id: H256,
    pub pool_address: H160,
    pub swap_fee_percentage: Bfp,
    pub amplification_parameter: BigRational,
    pub reserves: HashMap<H160, TokenState>,
    pub paused: bool,
}

impl StablePool {
    pub fn new(
        pool_data: RegisteredStablePool,
        balances: Vec<U256>,
        swap_fee_percentage: Bfp,
        amplification_parameter: BigRational,
        paused: bool,
    ) -> Self {
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
        StablePool {
            pool_id: pool_data.common.pool_id,
            pool_address: pool_data.common.pool_address,
            swap_fee_percentage,
            amplification_parameter,
            reserves,
            paused,
        }
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait BalancerPoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<BalancerPool>>;
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
        metrics: Arc<dyn BalancerPoolCacheMetrics>,
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
            RecentBlockCache::new(config, reserve_fetcher, block_stream, metrics)?;
        Ok(Self {
            pool_registry,
            pool_reserve_cache,
        })
    }
}

#[async_trait::async_trait]
impl BalancerPoolFetching for BalancerPoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<BalancerPool>> {
        let pool_ids = self
            .pool_registry
            .get_pool_ids_containing_token_pairs(token_pairs)
            .await;
        let fetched_pools = self.pool_reserve_cache.fetch(pool_ids, at_block).await?;
        // Return only those pools which are not paused.
        Ok(filter_paused(fetched_pools))
    }
}

fn filter_paused(pools: Vec<BalancerPool>) -> Vec<BalancerPool> {
    pools.into_iter().filter(|pool| !pool.paused()).collect()
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
    use maplit::{hashmap, hashset};

    #[test]
    fn filters_paused_pools() {
        let pools = vec![
            BalancerPool::Weighted(WeightedPool {
                pool_id: H256::from_low_u64_be(0),
                pool_address: Default::default(),
                swap_fee_percentage: Bfp::zero(),
                reserves: Default::default(),
                paused: true,
            }),
            BalancerPool::Stable(StablePool {
                pool_id: H256::from_low_u64_be(1),
                pool_address: Default::default(),
                swap_fee_percentage: Bfp::zero(),
                amplification_parameter: BigRational::new(10.into(), 1000.into()),
                reserves: Default::default(),
                paused: false,
            }),
        ];

        let filtered_pools = filter_paused(pools.clone());
        assert_eq!(filtered_pools.len(), 1);
        assert_eq!(filtered_pools[0].pool_id(), pools[1].pool_id());
    }

    #[test]
    fn try_into_stable_and_weighted() {
        let weighted_pool = BalancerPool::Weighted(WeightedPool {
            pool_id: H256::from_low_u64_be(0),
            pool_address: Default::default(),
            swap_fee_percentage: Bfp::zero(),
            reserves: Default::default(),
            paused: true,
        });

        assert!(weighted_pool.try_into_weighted().is_ok());
        assert!(weighted_pool.try_into_stable().is_err());

        let stable_pool = BalancerPool::Stable(StablePool {
            pool_id: H256::from_low_u64_be(1),
            pool_address: Default::default(),
            swap_fee_percentage: Bfp::zero(),
            amplification_parameter: BigRational::new(1.into(), 2.into()),
            reserves: Default::default(),
            paused: false,
        });

        assert!(stable_pool.try_into_stable().is_ok());
        assert!(stable_pool.try_into_weighted().is_err());
    }

    #[test]
    fn balancer_pool_state_helpers() {
        let weighted_pool_state = BalancerPoolState::Weighted(WeightedTokenState {
            token_state: TokenState {
                balance: U256::one(),
                scaling_exponent: 1,
            },
            weight: Bfp::one(),
        });
        assert_eq!(weighted_pool_state.scaling_exponent(), 1);
        assert_eq!(weighted_pool_state.balance(), U256::one());

        let stable_pool_state = BalancerPoolState::Stable(TokenState {
            balance: U256::zero(),
            scaling_exponent: 0,
        });
        assert_eq!(stable_pool_state.scaling_exponent(), 0);
        assert_eq!(stable_pool_state.balance(), U256::zero());
    }

    #[test]
    fn balancer_pool_helpers() {
        // Test all the helpers on Weighted Balancer Pools
        let weighted_pool_state = WeightedTokenState {
            token_state: TokenState {
                balance: U256::zero(),
                scaling_exponent: 0,
            },
            weight: Bfp::zero(),
        };
        let weighted_pool = BalancerPool::Weighted(WeightedPool {
            pool_id: H256::from_low_u64_be(1),
            pool_address: H160::from_low_u64_be(1),
            swap_fee_percentage: Bfp::zero(),
            reserves: hashmap! { H160::from_low_u64_be(1) => weighted_pool_state.clone()},
            paused: false,
        });
        assert!(weighted_pool.is_weighted());
        assert_eq!(weighted_pool.pool_id(), H256::from_low_u64_be(1));
        assert!(!weighted_pool.paused());
        assert_eq!(
            weighted_pool.reserve_keys(),
            hashset! { H160::from_low_u64_be(1) }
        );
        assert_eq!(weighted_pool.swap_fee_percentage(), Bfp::zero());
        assert_eq!(weighted_pool.pool_type(), PoolType::Weighted);
        assert_eq!(
            weighted_pool.reserves(),
            hashmap! { H160::from_low_u64_be(1) => BalancerPoolState::Weighted(weighted_pool_state) }
        );

        // Test all the helpers on Stable Balancer Pools
        let stable_pool_state = TokenState {
            balance: U256::one(),
            scaling_exponent: 1,
        };
        let stable_pool = BalancerPool::Stable(StablePool {
            pool_id: H256::from_low_u64_be(2),
            pool_address: H160::from_low_u64_be(2),
            swap_fee_percentage: Bfp::one(),
            amplification_parameter: BigRational::from_integer(2.into()),
            reserves: hashmap! { H160::from_low_u64_be(2) => stable_pool_state.clone() },
            paused: true,
        });
        assert!(!stable_pool.is_weighted());
        assert_eq!(stable_pool.pool_id(), H256::from_low_u64_be(2));
        assert!(stable_pool.paused());
        assert_eq!(
            stable_pool.reserve_keys(),
            hashset! { H160::from_low_u64_be(2) }
        );
        assert_eq!(stable_pool.swap_fee_percentage(), Bfp::one());
        assert_eq!(stable_pool.pool_type(), PoolType::Stable);
        assert_eq!(
            stable_pool.reserves(),
            hashmap! { H160::from_low_u64_be(2) => BalancerPoolState::Stable(stable_pool_state) }
        );
    }
}
