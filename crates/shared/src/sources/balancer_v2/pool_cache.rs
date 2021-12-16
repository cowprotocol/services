use super::{
    event_handler::BalancerPoolRegistry,
    pool_fetching::{AmplificationParameter, BalancerPoolEvaluating, StablePool, WeightedPool},
    pools::common,
};
use crate::{
    ethcontract_error::EthcontractErrorType,
    recent_block_cache::{Block, CacheFetching, CacheKey, CacheMetrics, RecentBlockCache},
    transport::MAX_BATCH_SIZE,
    Web3,
};
use anyhow::Result;
use contracts::BalancerV2StablePool;
use ethcontract::{batch::CallBatch, errors::MethodError, BlockId, H256};
use futures::future;
use std::{collections::HashSet, sync::Arc};

pub struct PoolReserveFetcher {
    pool_registry: Arc<BalancerPoolRegistry>,
    common_pool_fetcher: Arc<dyn common::PoolInfoFetching>,
    web3: Web3,
}

pub trait BalancerPoolCacheMetrics: Send + Sync {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize);
}

impl PoolReserveFetcher {
    pub async fn new(
        pool_registry: Arc<BalancerPoolRegistry>,
        common_pool_fetcher: Arc<dyn common::PoolInfoFetching>,
        web3: Web3,
    ) -> Result<Self> {
        Ok(Self {
            pool_registry,
            common_pool_fetcher,
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
        value.properties().id
    }
}

impl CacheKey<WeightedPool> for H256 {
    fn first_ord() -> Self {
        H256::zero()
    }

    fn for_value(value: &WeightedPool) -> Self {
        value.properties().id
    }
}

#[async_trait::async_trait]
impl CacheFetching<H256, WeightedPool> for PoolReserveFetcher {
    async fn fetch_values(
        &self,
        pool_ids: HashSet<H256>,
        at_block: Block,
    ) -> Result<Vec<WeightedPool>> {
        let mut batch = CallBatch::new(self.web3.transport().clone());
        let block = BlockId::Number(at_block.into());
        let weighted_pool_futures = self
            .pool_registry
            .get_weighted_pools(&pool_ids)
            .await
            .into_iter()
            .map(|registered_pool| {
                let common_pool_state = self.common_pool_fetcher.fetch_common_pool_state(
                    &registered_pool.common,
                    &mut batch,
                    block,
                );

                async move { Ok(WeightedPool::new(registered_pool, common_pool_state.await?)) }
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        let results = future::join_all(weighted_pool_futures).await;
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
        let mut batch = CallBatch::new(self.web3.transport().clone());
        let block = BlockId::Number(at_block.into());
        let futures = self
            .pool_registry
            .get_stable_pools(&pool_ids)
            .await
            .into_iter()
            .map(|registered_pool| {
                let common_pool_state = self.common_pool_fetcher.fetch_common_pool_state(
                    &registered_pool.common,
                    &mut batch,
                    block,
                );

                let pool_contract =
                    BalancerV2StablePool::at(&self.web3, registered_pool.common.address);
                let amplification_parameter = pool_contract
                    .get_amplification_parameter()
                    .block(block)
                    .batch_call(&mut batch);

                async move {
                    let common = common_pool_state.await?;
                    let amplification_parameter = {
                        let (factor, _, precision) = amplification_parameter.await?;
                        AmplificationParameter::new(factor, precision)?
                    };

                    Ok(StablePool::new(
                        registered_pool,
                        common,
                        amplification_parameter,
                    ))
                }
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;

        let results = future::join_all(futures).await;
        accumulate_handled_results(results)
    }
}

impl CacheMetrics for Arc<dyn BalancerPoolCacheMetrics> {
    fn entries_fetched(&self, cache_hits: usize, cache_misses: usize) {
        self.pools_fetched(cache_hits, cache_misses)
    }
}

fn accumulate_handled_results<T>(results: Vec<Result<T>>) -> Result<Vec<T>> {
    results
        .into_iter()
        .filter_map(|result| match result {
            Ok(value) => Some(Ok(value)),
            Err(err) if is_contract_error(&err) => None,
            Err(err) => Some(Err(err)),
        })
        .collect()
}

fn is_contract_error(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<MethodError>()
            .map(EthcontractErrorType::classify),
        Some(EthcontractErrorType::Contract),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethcontract_error;

    #[test]
    fn pool_fetcher_forwards_node_error() {
        assert!(accumulate_handled_results(vec![
            Ok(()),
            Err(ethcontract_error::testing_node_error().into()),
        ])
        .is_err())
    }

    #[test]
    fn pool_fetcher_skips_contract_error() {
        assert_eq!(
            accumulate_handled_results(vec![
                Ok(()),
                Err(ethcontract_error::testing_contract_error().into()),
            ])
            .unwrap()
            .len(),
            1
        )
    }
}
