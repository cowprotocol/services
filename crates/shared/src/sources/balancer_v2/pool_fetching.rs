//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `Weighted` and `Stable`
//! pools to be consumed by external users (e.g. Price Estimators and Solvers).
use crate::{
    conversions::U256Ext,
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig, RecentBlockCache},
    sources::balancer_v2::{
        event_handler::BalancerPoolRegistry,
        info_fetching::PoolInfoFetcher,
        pool_cache::{
            BalancerPoolCacheMetrics, PoolReserveFetcher, StablePoolReserveCache,
            WeightedPoolReserveCache,
        },
        pool_init::DefaultPoolInitializer,
        pool_storage::{RegisteredStablePool, RegisteredWeightedPool},
        swap::fixed_point::Bfp,
    },
    token_info::TokenInfoFetching,
    Web3,
};
use anyhow::{ensure, Result};
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

pub trait BalancerPoolEvaluating {
    fn properties(&self) -> CommonPoolState;
}

#[derive(Clone, Debug)]
pub struct CommonPoolState {
    pub pool_id: H256,
    pub pool_address: H160, // This one isn't actually used (yet)
    pub swap_fee_percentage: Bfp,
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
            common: CommonPoolState {
                pool_id: pool_data.common.pool_id,
                pool_address: pool_data.common.pool_address,
                swap_fee_percentage,
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

#[derive(Clone, Debug, PartialEq)]
pub struct AmplificationParameter {
    factor: U256,
    precision: U256,
}

impl AmplificationParameter {
    pub fn new(factor: U256, precision: U256) -> Result<Self> {
        ensure!(!precision.is_zero(), "Zero precision not allowed");
        Ok(Self { factor, precision })
    }

    /// This is the format used to pass into smart contracts.
    pub fn as_u256(&self) -> U256 {
        self.factor * self.precision
    }

    /// This is the format used to pass along to HTTP solver.
    pub fn as_big_rational(&self) -> BigRational {
        // We can assert that the precision is non-zero as we check when constructing
        // new `AmplificationParameter` instances that this invariant holds, and we don't
        // allow modifications of `self.precision` such that it could become 0.
        debug_assert!(!self.precision.is_zero());
        BigRational::new(self.factor.to_big_int(), self.precision.to_big_int())
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
        balances: Vec<U256>,
        swap_fee_percentage: Bfp,
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
                pool_id: pool_data.common.pool_id,
                pool_address: pool_data.common.pool_address,
                swap_fee_percentage,
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
        let pool_info = Arc::new(PoolInfoFetcher {
            web3: web3.clone(),
            token_info_fetcher: token_info_fetcher.clone(),
            vault: BalancerV2Vault::deployed(&web3).await?,
        });
        let pool_initializer = DefaultPoolInitializer::new(chain_id, pool_info.clone(), client)?;
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
            .get_pool_ids_containing_token_pairs(token_pairs)
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
                    pool_id: H256::from_low_u64_be(0),
                    pool_address: Default::default(),
                    swap_fee_percentage: Bfp::zero(),
                    paused: true,
                },
                reserves: Default::default(),
            },
            WeightedPool {
                common: CommonPoolState {
                    pool_id: H256::from_low_u64_be(1),
                    pool_address: Default::default(),
                    swap_fee_percentage: Bfp::zero(),
                    paused: false,
                },
                reserves: Default::default(),
            },
        ];

        let filtered_pools = filter_paused(pools.clone());
        assert_eq!(filtered_pools.len(), 1);
        assert_eq!(filtered_pools[0].common.pool_id, pools[1].common.pool_id);
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
