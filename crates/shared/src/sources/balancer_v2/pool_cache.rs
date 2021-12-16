use super::{
    event_handler::BalancerPoolRegistry,
    pool_fetching::{BalancerPoolEvaluating, StablePool, WeightedPool},
    pools::{common, FactoryIndexing},
};
use crate::{
    ethcontract_error::EthcontractErrorType,
    recent_block_cache::{Block, CacheFetching, CacheKey, CacheMetrics, RecentBlockCache},
    transport::MAX_BATCH_SIZE,
    Web3,
};
use anyhow::Result;
use contracts::{BalancerV2StablePoolFactory, BalancerV2WeightedPoolFactory};
use ethcontract::{batch::CallBatch, errors::MethodError, BlockId, H256};
use futures::{future, FutureExt as _};
use std::{collections::HashSet, future::Future, sync::Arc};
use tokio::sync::oneshot;

pub struct PoolReserveFetcher {
    pool_registry: Arc<BalancerPoolRegistry>,
    common_pool_fetcher: Arc<dyn common::PoolInfoFetching>,
    weighted_pool_factory: BalancerV2WeightedPoolFactory,
    stable_pool_factory: BalancerV2StablePoolFactory,
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
        let weighted_pool_factory = BalancerV2WeightedPoolFactory::deployed(&web3).await?;
        let stable_pool_factory = BalancerV2StablePoolFactory::deployed(&web3).await?;

        Ok(Self {
            pool_registry,
            common_pool_fetcher,
            weighted_pool_factory,
            stable_pool_factory,
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
                let (common_pool_state, common_pool_state_ok) =
                    share_common_pool_state(self.common_pool_fetcher.fetch_common_pool_state(
                        &registered_pool.common,
                        &mut batch,
                        block,
                    ));
                let weighted_pool_state = self.weighted_pool_factory.fetch_pool_state(
                    &registered_pool,
                    common_pool_state_ok.boxed(),
                    &mut batch,
                    block,
                );

                async move {
                    let common_pool_state = common_pool_state.await?;
                    let weighted_pool_state = weighted_pool_state.await?;
                    Ok(WeightedPool::new(
                        registered_pool,
                        common_pool_state,
                        weighted_pool_state,
                    ))
                }
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
                let (common_pool_state, common_pool_state_ok) =
                    share_common_pool_state(self.common_pool_fetcher.fetch_common_pool_state(
                        &registered_pool.common,
                        &mut batch,
                        block,
                    ));
                let stable_pool_state = self.stable_pool_factory.fetch_pool_state(
                    &registered_pool,
                    common_pool_state_ok.boxed(),
                    &mut batch,
                    block,
                );

                async move {
                    let common_pool_state = common_pool_state.await?;
                    let stable_pool_state = stable_pool_state.await?;

                    Ok(StablePool::new(
                        registered_pool,
                        common_pool_state,
                        stable_pool_state,
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

/// An internal utility method for sharing the success value for an
/// `anyhow::Result`.
///
/// Typically, this is pretty trivial using `FutureExt::shared`. However, since
/// `anyhow::Error: !Clone` we need to use a different approach.
///
/// # Panics
///
/// Polling the future with the shared success value will panic if the result
/// future has not already resolved to a `Ok` value. This method is only ever
/// meant to be used internally, so we don't have to worry that these
/// assumptions leak out of this module.
fn share_common_pool_state(
    fut: impl Future<Output = Result<common::PoolState>>,
) -> (
    impl Future<Output = Result<common::PoolState>>,
    impl Future<Output = common::PoolState>,
) {
    let (pool_sender, pool_receiver) = oneshot::channel();

    let result = fut.inspect(|pool_result| {
        // We can't clone `anyhow::Error` so just clone the pool data and use
        // an empty `()` error.
        let pool_result = pool_result.as_ref().map(Clone::clone).map_err(|_| ());
        // Ignore error if the shared future was dropped.
        let _ = pool_sender.send(pool_result);
    });
    let shared = async move {
        pool_receiver
            .now_or_never()
            .expect("result future is still pending")
            .expect("result future was dropped")
            .expect("result future resolved to an error")
    };

    (result, shared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethcontract_error;
    use anyhow::bail;

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

    #[tokio::test]
    async fn share_pool_state_future() {
        let (pool_state, pool_state_ok) = share_common_pool_state(async { Ok(Default::default()) });
        assert_eq!({ pool_state.await.unwrap() }, pool_state_ok.await);
    }

    #[tokio::test]
    #[should_panic]
    async fn shared_pool_state_future_panics_if_pending() {
        let (_pool_state, pool_state_ok) = share_common_pool_state(async {
            futures::pending!();
            Ok(Default::default())
        });
        pool_state_ok.await;
    }

    #[tokio::test]
    #[should_panic]
    async fn share_pool_state_future_if_dropped() {
        let (pool_state, pool_state_ok) = share_common_pool_state(async { Ok(Default::default()) });
        drop(pool_state);
        pool_state_ok.await;
    }

    #[tokio::test]
    #[should_panic]
    async fn share_pool_state_future_if_errored() {
        let (pool_state, pool_state_ok) = share_common_pool_state(async { bail!("error") });
        let _ = pool_state.await;
        pool_state_ok.await;
    }
}
