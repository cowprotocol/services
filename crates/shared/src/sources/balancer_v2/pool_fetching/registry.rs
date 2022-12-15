//! A pool registry for a single pool factory that is generic on its type of
//! pool.

use super::{internal::InternalPoolFetching, pool_storage::PoolStorage};
use crate::{
    current_block::{BlockNumberHash, BlockRetrieving},
    ethcontract_error::EthcontractErrorType,
    ethrpc::{Web3, Web3CallBatch, Web3Transport, MAX_BATCH_SIZE},
    event_handling::EventHandler,
    impl_event_retrieving,
    maintenance::{Maintainer, Maintaining},
    recent_block_cache::Block,
    sources::balancer_v2::pools::{common::PoolInfoFetching, FactoryIndexing, Pool, PoolStatus},
};
use anyhow::Result;
use contracts::{balancer_v2_base_pool_factory, BalancerV2BasePoolFactory};
use ethcontract::{errors::MethodError, BlockId, Instance, H256};
use futures::future;
use model::TokenPair;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;

impl_event_retrieving! {
    pub BasePoolFactoryContract for balancer_v2_base_pool_factory
}

/// Type alias for the internal event updater type.
type PoolUpdater<Factory> = Mutex<EventHandler<BasePoolFactoryContract, PoolStorage<Factory>>>;

/// The Pool Registry maintains an event handler for each of the Balancer Pool Factory contracts
/// and maintains a `PoolStorage` for each.
/// Pools are read from this registry, via the public method `pool_ids_for_token_pairs`
/// which takes a collection of `TokenPair`, gets the relevant pools from each `PoolStorage`
/// and returns a merged de-duplicated version of the results.
pub struct Registry<Factory>
where
    Factory: FactoryIndexing,
{
    web3: Web3,
    fetcher: Arc<dyn PoolInfoFetching<Factory>>,
    updater: PoolUpdater<Factory>,
}

impl<Factory> Registry<Factory>
where
    Factory: FactoryIndexing,
{
    /// Returns a new pool registry for the specified factory.
    pub fn new(
        block_retreiver: Arc<dyn BlockRetrieving>,
        fetcher: Arc<dyn PoolInfoFetching<Factory>>,
        factory_instance: &Instance<Web3Transport>,
        initial_pools: Vec<Factory::PoolInfo>,
        start_sync_at_block: Option<BlockNumberHash>,
    ) -> Self {
        let web3 = factory_instance.web3();
        let updater = Mutex::new(EventHandler::new(
            block_retreiver,
            BasePoolFactoryContract(base_pool_factory(factory_instance)),
            PoolStorage::new(initial_pools, fetcher.clone()),
            start_sync_at_block,
        ));
        Self {
            web3,
            fetcher,
            updater,
        }
    }
}

#[async_trait::async_trait]
impl<Factory> InternalPoolFetching for Registry<Factory>
where
    Factory: FactoryIndexing,
{
    async fn pool_ids_for_token_pairs(&self, token_pairs: HashSet<TokenPair>) -> HashSet<H256> {
        self.updater
            .lock()
            .await
            .store()
            .pool_ids_for_token_pairs(&token_pairs)
    }

    async fn pools_by_id(&self, pool_ids: HashSet<H256>, block: Block) -> Result<Vec<Pool>> {
        let mut batch = Web3CallBatch::new(self.web3.transport().clone());
        let block = BlockId::Number(block.into());

        let pool_infos = self.updater.lock().await.store().pools_by_id(&pool_ids);
        let pool_futures = pool_infos
            .into_iter()
            .map(|pool_info| self.fetcher.fetch_pool(&pool_info, &mut batch, block))
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;

        let pools = future::join_all(pool_futures).await;
        collect_pool_results(pools)
    }
}

#[async_trait::async_trait]
impl<Factory> Maintaining for Registry<Factory>
where
    Factory: FactoryIndexing,
{
    async fn run_maintenance(&self) -> Result<()> {
        self.updater.run_maintenance().await
    }

    fn name(&self) -> Maintainer {
        Maintainer::BalancerPoolFetcher
    }
}

fn base_pool_factory(contract_instance: &Instance<Web3Transport>) -> BalancerV2BasePoolFactory {
    BalancerV2BasePoolFactory::with_deployment_info(
        &contract_instance.web3(),
        contract_instance.address(),
        contract_instance.deployment_information(),
    )
}

fn collect_pool_results(pools: Vec<Result<PoolStatus>>) -> Result<Vec<Pool>> {
    pools
        .into_iter()
        .filter_map(|pool| match pool {
            Ok(pool) => Some(Ok(pool.active()?)),
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
    use crate::{
        ethcontract_error,
        sources::balancer_v2::{
            pools::{weighted, PoolKind},
            swap::fixed_point::Bfp,
        },
    };

    #[tokio::test]
    async fn collecting_results_filters_paused_pools_and_contract_errors() {
        let results = vec![
            Ok(PoolStatus::Active(Pool {
                id: Default::default(),
                kind: PoolKind::Weighted(weighted::PoolState {
                    tokens: Default::default(),
                    swap_fee: Bfp::zero(),
                }),
            })),
            Ok(PoolStatus::Paused),
            Err(ethcontract_error::testing_contract_error().into()),
        ];
        assert_eq!(collect_pool_results(results).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn collecting_results_forwards_node_error() {
        let node_err = Err(ethcontract_error::testing_node_error().into());
        assert!(collect_pool_results(vec![node_err]).is_err());
    }
}
