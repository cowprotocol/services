use crate::{
    recent_block_cache::{Block, CacheFetching, CacheKey, CacheMetrics, RecentBlockCache},
    sources::{
        balancer_v2::{
            event_handler::BalancerPoolRegistry,
            pool_fetching::{BalancerPoolEvaluating, StablePool, WeightedPool},
            pool_storage::{PoolEvaluating, RegisteredStablePool, RegisteredWeightedPool},
            swap::fixed_point::Bfp,
        },
        uniswap_v2::pool_fetching::handle_contract_error,
    },
    transport::MAX_BATCH_SIZE,
    Web3,
};
use anyhow::Result;
use contracts::{BalancerV2StablePool, BalancerV2Vault, BalancerV2WeightedPool};
use ethcontract::{batch::CallBatch, errors::MethodError, BlockId, Bytes, H160, H256, U256};
use std::{collections::HashSet, sync::Arc};

pub struct PoolReserveFetcher {
    pool_registry: Arc<BalancerPoolRegistry>,
    vault: BalancerV2Vault,
    web3: Web3,
}

pub trait BalancerPoolCacheMetrics: Send + Sync {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize);
}

impl PoolReserveFetcher {
    pub async fn new(pool_registry: Arc<BalancerPoolRegistry>, web3: Web3) -> Result<Self> {
        let vault = BalancerV2Vault::deployed(&web3).await?;
        Ok(Self {
            pool_registry,
            vault,
            web3,
        })
    }
}

pub type WeightedPoolReserveCache =
    RecentBlockCache<H256, WeightedPool, PoolReserveFetcher, Arc<dyn BalancerPoolCacheMetrics>>;

pub type StablePoolReserveCache =
    RecentBlockCache<H256, StablePool, PoolReserveFetcher, Arc<dyn BalancerPoolCacheMetrics>>;

impl CacheKey<StablePool> for H256 {
    fn first_ord() -> Self {
        H256::zero()
    }

    fn for_value(value: &StablePool) -> Self {
        value.properties().pool_id
    }
}

impl CacheKey<WeightedPool> for H256 {
    fn first_ord() -> Self {
        H256::zero()
    }

    fn for_value(value: &WeightedPool) -> Self {
        value.properties().pool_id
    }
}

#[async_trait::async_trait]
impl CacheFetching<H256, WeightedPool> for PoolReserveFetcher {
    async fn fetch_values(
        &self,
        pool_ids: HashSet<H256>,
        at_block: Block,
    ) -> Result<Vec<WeightedPool>> {
        let mut batch = CallBatch::new(self.web3.transport());
        let block = BlockId::Number(at_block.into());
        let weighted_pool_futures = self
            .pool_registry
            .get_weighted_pools(&pool_ids)
            .await
            .into_iter()
            .map(|registered_pool| {
                let pool_contract =
                    BalancerV2WeightedPool::at(&self.web3, registered_pool.common.pool_address);
                let swap_fee = pool_contract
                    .get_swap_fee_percentage()
                    .block(block)
                    .batch_call(&mut batch);
                let reserves = self
                    .vault
                    .get_pool_tokens(Bytes(registered_pool.common.pool_id.0))
                    .block(block)
                    .batch_call(&mut batch);
                let paused_state = pool_contract
                    .get_paused_state()
                    .block(block)
                    .batch_call(&mut batch);
                async move {
                    #[allow(clippy::eval_order_dependence)]
                    FetchedWeightedPool {
                        registered_pool,
                        common: FetchedCommonPool {
                            swap_fee_percentage: swap_fee.await,
                            reserves: reserves.await,
                            paused_state: paused_state.await,
                        },
                    }
                }
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        let mut results: Vec<FetchedWeightedPool> = Vec::new();
        for future in weighted_pool_futures {
            // Batch has already been executed, so these awaits resolve immediately.
            results.push(future.await);
        }

        accumulate_handled_results(results)
    }
}

#[async_trait::async_trait]
impl CacheFetching<H256, StablePool> for PoolReserveFetcher {
    async fn fetch_values(
        &self,
        pool_ids: HashSet<H256>,
        at_block: Block,
    ) -> Result<Vec<StablePool>> {
        let mut batch = CallBatch::new(self.web3.transport());
        let block = BlockId::Number(at_block.into());
        let futures = self
            .pool_registry
            .get_stable_pools(&pool_ids)
            .await
            .into_iter()
            .map(|registered_pool| {
                let pool_contract =
                    BalancerV2StablePool::at(&self.web3, registered_pool.properties().pool_address);
                let swap_fee = pool_contract
                    .get_swap_fee_percentage()
                    .block(block)
                    .batch_call(&mut batch);
                let reserves = self
                    .vault
                    .get_pool_tokens(Bytes(registered_pool.properties().pool_id.0))
                    .block(block)
                    .batch_call(&mut batch);
                let paused_state = pool_contract
                    .get_paused_state()
                    .block(block)
                    .batch_call(&mut batch);
                let amplification_parameter = pool_contract
                    .get_amplification_parameter()
                    .block(block)
                    .batch_call(&mut batch);
                async move {
                    #[allow(clippy::eval_order_dependence)]
                    FetchedStablePool {
                        registered_pool,
                        common: FetchedCommonPool {
                            swap_fee_percentage: swap_fee.await,
                            reserves: reserves.await,
                            paused_state: paused_state.await,
                        },
                        amplification_parameter: amplification_parameter.await,
                    }
                }
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        let mut results: Vec<FetchedStablePool> = Vec::new();

        for future in futures {
            // Batch has already been executed, so these awaits resolve immediately.
            results.push(future.await);
        }

        accumulate_handled_results(results)
    }
}

impl CacheMetrics for Arc<dyn BalancerPoolCacheMetrics> {
    fn entries_fetched(&self, cache_hits: usize, cache_misses: usize) {
        self.pools_fetched(cache_hits, cache_misses)
    }
}

struct FetchedCommonPool {
    swap_fee_percentage: Result<U256, MethodError>,
    /// getPoolTokens returns (Tokens, Balances, LastBlockUpdated)
    reserves: Result<(Vec<H160>, Vec<U256>, U256), MethodError>,
    /// getPausedState returns (paused, pauseWindowEndTime, bufferPeriodEndTime)
    paused_state: Result<(bool, U256, U256), MethodError>,
}

/// An internal temporary struct used during pool fetching to handle errors.
struct FetchedWeightedPool {
    registered_pool: RegisteredWeightedPool,
    common: FetchedCommonPool,
}

struct FetchedStablePool {
    registered_pool: RegisteredStablePool,
    common: FetchedCommonPool,
    /// getAmplificationParameter returns (value, isUpdating, precision)
    amplification_parameter: Result<(U256, bool, U256), MethodError>,
}

pub trait FetchedBalancerPoolConverting<T> {
    fn handle_results(self) -> Result<Option<T>>;
}

fn accumulate_handled_results<T>(
    results: Vec<impl FetchedBalancerPoolConverting<T>>,
) -> Result<Vec<T>> {
    results
        .into_iter()
        .try_fold(Vec::new(), |mut acc, fetched_pool| {
            match fetched_pool.handle_results()? {
                None => return Ok(acc),
                Some(pool) => acc.push(pool),
            }
            Ok(acc)
        })
}

impl FetchedBalancerPoolConverting<CommonFetchedPoolInfo> for FetchedCommonPool {
    fn handle_results(self) -> Result<Option<CommonFetchedPoolInfo>> {
        let balances = match handle_contract_error(self.reserves)? {
            // We only keep the balances entry of reserves query.
            Some(reserves) => reserves.1,
            None => return Ok(None),
        };
        let swap_fee_percentage = match handle_contract_error(self.swap_fee_percentage)? {
            Some(swap_fee) => swap_fee,
            None => return Ok(None),
        };
        let paused = match handle_contract_error(self.paused_state)? {
            // We only keep the boolean value regarding whether the pool is paused or not
            Some(state) => state.0,
            None => return Ok(None),
        };

        Ok(Some((balances, swap_fee_percentage, paused)))
    }
}

type CommonFetchedPoolInfo = (Vec<U256>, U256, bool);

impl FetchedBalancerPoolConverting<StablePool> for FetchedStablePool {
    fn handle_results(self) -> Result<Option<StablePool>> {
        let (balances, swap_fee_percentage, paused) = match self.common.handle_results()? {
            Some(results) => results,
            None => return Ok(None),
        };

        let (amplification_factor, amplification_precision) =
            match handle_contract_error(self.amplification_parameter)? {
                Some((factor, _, precision)) => (factor, precision),
                None => return Ok(None),
            };
        let result = StablePool::new(
            self.registered_pool,
            balances,
            Bfp::from_wei(swap_fee_percentage),
            amplification_factor,
            amplification_precision,
            paused,
        )?;
        Ok(Some(result))
    }
}

impl FetchedBalancerPoolConverting<WeightedPool> for FetchedWeightedPool {
    fn handle_results(self) -> Result<Option<WeightedPool>> {
        let (balances, swap_fee_percentage, paused) = match self.common.handle_results()? {
            Some(results) => results,
            None => return Ok(None),
        };

        Ok(Some(WeightedPool::new(
            self.registered_pool,
            balances,
            Bfp::from_wei(swap_fee_percentage),
            paused,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ethcontract_error, sources::balancer_v2::pool_storage::RegisteredWeightedPool};

    #[test]
    fn pool_fetcher_forwards_node_error() {
        let fetched_weighted_pool = FetchedWeightedPool {
            registered_pool: RegisteredWeightedPool::default(),
            common: FetchedCommonPool {
                swap_fee_percentage: Ok(U256::zero()),
                reserves: Err(ethcontract_error::testing_node_error()),
                paused_state: Ok((true, U256::zero(), U256::zero())),
            },
        };
        assert!(fetched_weighted_pool.handle_results().is_err());
        let fetched_weighted_pool = FetchedWeightedPool {
            registered_pool: RegisteredWeightedPool::default(),
            common: FetchedCommonPool {
                swap_fee_percentage: Err(ethcontract_error::testing_node_error()),
                reserves: Ok((vec![], vec![], U256::zero())),
                paused_state: Ok((true, U256::zero(), U256::zero())),
            },
        };
        assert!(fetched_weighted_pool.handle_results().is_err());
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        let results = vec![
            FetchedWeightedPool {
                registered_pool: RegisteredWeightedPool::default(),
                common: FetchedCommonPool {
                    swap_fee_percentage: Ok(U256::zero()),
                    reserves: Err(ethcontract_error::testing_contract_error()),
                    paused_state: Ok((true, U256::zero(), U256::zero())),
                },
            },
            FetchedWeightedPool {
                registered_pool: RegisteredWeightedPool::default(),
                common: FetchedCommonPool {
                    swap_fee_percentage: Err(ethcontract_error::testing_contract_error()),
                    reserves: Ok((vec![], vec![], U256::zero())),
                    paused_state: Ok((true, U256::zero(), U256::zero())),
                },
            },
            FetchedWeightedPool {
                registered_pool: RegisteredWeightedPool::default(),
                common: FetchedCommonPool {
                    swap_fee_percentage: Ok(U256::zero()),
                    reserves: Ok((vec![], vec![], U256::zero())),
                    paused_state: Ok((true, U256::zero(), U256::zero())),
                },
            },
        ];
        assert_eq!(accumulate_handled_results(results).unwrap().len(), 1);
    }
}
