use crate::sources::balancer::pool_storage::{PoolEvaluating, RegisteredPool};
use crate::{
    recent_block_cache::{Block, CacheFetching, CacheKey, CacheMetrics, RecentBlockCache},
    sources::{
        balancer::{
            event_handler::BalancerPoolRegistry,
            pool_fetching::{BalancerPool, StablePool, WeightedPool},
            swap::fixed_point::Bfp,
        },
        uniswap::pool_fetching::{handle_contract_error, MAX_BATCH_SIZE},
    },
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

pub type BalancerPoolReserveCache =
    RecentBlockCache<H256, BalancerPool, PoolReserveFetcher, Arc<dyn BalancerPoolCacheMetrics>>;

impl CacheKey<BalancerPool> for H256 {
    fn first_ord() -> Self {
        H256::zero()
    }

    fn for_value(value: &BalancerPool) -> Self {
        value.pool_id()
    }
}

#[async_trait::async_trait]
impl CacheFetching<H256, BalancerPool> for PoolReserveFetcher {
    async fn fetch_values(
        &self,
        pool_ids: HashSet<H256>,
        at_block: Block,
    ) -> Result<Vec<BalancerPool>> {
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
                    FetchedBalancerPool {
                        registered_pool: RegisteredPool::Weighted(registered_pool),
                        swap_fee_percentage: swap_fee.await,
                        reserves: reserves.await,
                        paused_state: paused_state.await,
                        /// This value is irrelevant for weighted pools
                        amplification_parameter: None,
                    }
                }
            })
            .collect::<Vec<_>>();
        let stable_pool_futures = self
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
                    FetchedBalancerPool {
                        registered_pool: RegisteredPool::Stable(registered_pool),
                        swap_fee_percentage: swap_fee.await,
                        reserves: reserves.await,
                        paused_state: paused_state.await,
                        amplification_parameter: Some(amplification_parameter.await),
                    }
                }
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        let mut results = Vec::new();
        for future in weighted_pool_futures {
            // Batch has already been executed, so these awaits resolve immediately.
            results.push(future.await);
        }

        for future in stable_pool_futures {
            // Batch has already been executed, so these awaits resolve immediately.
            results.push(future.await);
        }
        handle_results(results)
    }
}

impl CacheMetrics for Arc<dyn BalancerPoolCacheMetrics> {
    fn entries_fetched(&self, cache_hits: usize, cache_misses: usize) {
        self.pools_fetched(cache_hits, cache_misses)
    }
}

/// An internal temporary struct used during pool fetching to handle errors.
struct FetchedBalancerPool {
    registered_pool: RegisteredPool,
    swap_fee_percentage: Result<U256, MethodError>,
    /// getPoolTokens returns (Tokens, Balances, LastBlockUpdated)
    reserves: Result<(Vec<H160>, Vec<U256>, U256), MethodError>,
    /// getPausedState returns (paused, pauseWindowEndTime, bufferPeriodEndTime)
    paused_state: Result<(bool, U256, U256), MethodError>,
    /// getAmplificationParameter returns (value, isUpdating, precision)
    /// Only relevant for Stable Pools.
    amplification_parameter: Option<Result<(U256, bool, U256), MethodError>>,
}

fn handle_results(results: Vec<FetchedBalancerPool>) -> Result<Vec<BalancerPool>> {
    results
        .into_iter()
        .try_fold(Vec::new(), |mut acc, fetched_pool| {
            let balances = match handle_contract_error(fetched_pool.reserves)? {
                // We only keep the balances entry of reserves query.
                Some(reserves) => reserves.1,
                None => return Ok(acc),
            };
            let swap_fee_percentage = match handle_contract_error(fetched_pool.swap_fee_percentage)?
            {
                Some(swap_fee) => swap_fee,
                None => return Ok(acc),
            };
            let paused = match handle_contract_error(fetched_pool.paused_state)? {
                // We only keep the boolean value regarding whether the pool is paused or not
                Some(state) => state.0,
                None => return Ok(acc),
            };
            match fetched_pool.registered_pool {
                RegisteredPool::Weighted(pool_data) => {
                    acc.push(BalancerPool::Weighted(WeightedPool::new(
                        pool_data,
                        balances,
                        Bfp::from_wei(swap_fee_percentage),
                        paused,
                    )));
                }
                RegisteredPool::Stable(pool_data) => {
                    let amplification_parameter = match handle_contract_error(
                        fetched_pool
                            .amplification_parameter
                            .expect("Stable pools must have this set."),
                    )? {
                        // We only keep the U256 value and disregard isUpdating and precision.
                        Some(state) => state.0,
                        None => return Ok(acc),
                    };
                    acc.push(BalancerPool::Stable(StablePool::new(
                        pool_data,
                        balances,
                        Bfp::from_wei(swap_fee_percentage),
                        amplification_parameter,
                        paused,
                    )));
                }
            }
            Ok(acc)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethcontract_error;
    use crate::sources::balancer::pool_storage::RegisteredWeightedPool;

    #[test]
    fn pool_fetcher_forwards_node_error() {
        let results = vec![FetchedBalancerPool {
            registered_pool: RegisteredPool::Weighted(RegisteredWeightedPool::default()),
            swap_fee_percentage: Ok(U256::zero()),
            reserves: Err(ethcontract_error::testing_node_error()),
            paused_state: Ok((true, U256::zero(), U256::zero())),
            amplification_parameter: None,
        }];
        assert!(handle_results(results).is_err());
        let results = vec![FetchedBalancerPool {
            registered_pool: RegisteredPool::Weighted(RegisteredWeightedPool::default()),
            swap_fee_percentage: Err(ethcontract_error::testing_node_error()),
            reserves: Ok((vec![], vec![], U256::zero())),
            paused_state: Ok((true, U256::zero(), U256::zero())),
            amplification_parameter: None,
        }];
        assert!(handle_results(results).is_err());
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        let results = vec![
            FetchedBalancerPool {
                registered_pool: RegisteredPool::Weighted(RegisteredWeightedPool::default()),
                swap_fee_percentage: Ok(U256::zero()),
                reserves: Err(ethcontract_error::testing_contract_error()),
                paused_state: Ok((true, U256::zero(), U256::zero())),
                amplification_parameter: None,
            },
            FetchedBalancerPool {
                registered_pool: RegisteredPool::Weighted(RegisteredWeightedPool::default()),
                swap_fee_percentage: Err(ethcontract_error::testing_contract_error()),
                reserves: Ok((vec![], vec![], U256::zero())),
                paused_state: Ok((true, U256::zero(), U256::zero())),
                amplification_parameter: None,
            },
            FetchedBalancerPool {
                registered_pool: RegisteredPool::Weighted(RegisteredWeightedPool::default()),
                swap_fee_percentage: Ok(U256::zero()),
                reserves: Ok((vec![], vec![], U256::zero())),
                paused_state: Ok((true, U256::zero(), U256::zero())),
                amplification_parameter: None,
            },
        ];
        assert_eq!(handle_results(results).unwrap().len(), 1);
    }
}
