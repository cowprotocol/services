//! A pool registry for a single pool factory that is generic on its type of
//! pool.

use {
    super::{internal::InternalPoolFetching, pool_storage::PoolStorage},
    crate::{
        event_handling::{AlloyEventRetrieving, EventHandler},
        maintenance::Maintaining,
        recent_block_cache::Block,
        sources::balancer_v2::{
            pool_fetching::BalancerFactoryInstance,
            pools::{FactoryIndexing, Pool, PoolStatus, common::PoolInfoFetching},
        },
    },
    BalancerV2BasePoolFactory::BalancerV2BasePoolFactory::BalancerV2BasePoolFactoryEvents,
    alloy::{
        primitives::B256,
        providers::DynProvider,
        rpc::types::{Filter, FilterSet, Log},
        sol_types::SolEvent,
    },
    anyhow::Result,
    contracts::BalancerV2BasePoolFactory::{self, BalancerV2BasePoolFactory::PoolCreated},
    ethrpc::{
        alloy::errors::ContractErrorExt,
        block_stream::{BlockNumberHash, BlockRetrieving},
    },
    futures::future,
    model::TokenPair,
    std::{collections::HashSet, sync::Arc},
    tokio::sync::Mutex,
};

pub struct BasePoolFactoryContract(BalancerV2BasePoolFactory::Instance);

#[async_trait::async_trait]
impl AlloyEventRetrieving for BasePoolFactoryContract {
    type Event = BalancerV2BasePoolFactoryEvents;

    fn provider(&self) -> &DynProvider {
        self.0.provider()
    }

    fn filter(&self) -> Filter {
        Filter::new()
            .event_signature(FilterSet::from_iter([PoolCreated::SIGNATURE_HASH]))
            .address(*self.0.address())
    }
}

/// Type alias for the internal event updater type.
type PoolUpdater<Factory> = Mutex<
    EventHandler<
        BasePoolFactoryContract,
        PoolStorage<Factory>,
        (BalancerV2BasePoolFactoryEvents, Log),
    >,
>;

/// The Pool Registry maintains an event handler for each of the Balancer Pool
/// Factory contracts and maintains a `PoolStorage` for each.
/// Pools are read from this registry, via the public method
/// `pool_ids_for_token_pairs` which takes a collection of `TokenPair`, gets the
/// relevant pools from each `PoolStorage` and returns a merged de-duplicated
/// version of the results.
pub struct Registry<Factory>
where
    Factory: FactoryIndexing,
{
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
        factory_instance: &BalancerFactoryInstance,
        initial_pools: Vec<Factory::PoolInfo>,
        start_sync_at_block: Option<BlockNumberHash>,
    ) -> Self {
        let updater = Mutex::new(EventHandler::new(
            block_retreiver,
            BasePoolFactoryContract(base_pool_factory(factory_instance)),
            PoolStorage::new(initial_pools, fetcher.clone()),
            start_sync_at_block,
        ));
        Self { fetcher, updater }
    }
}

#[async_trait::async_trait]
impl<Factory> InternalPoolFetching for Registry<Factory>
where
    Factory: FactoryIndexing,
{
    async fn pool_ids_for_token_pairs(&self, token_pairs: HashSet<TokenPair>) -> HashSet<B256> {
        self.updater
            .lock()
            .await
            .store()
            .pool_ids_for_token_pairs(&token_pairs)
    }

    async fn pools_by_id(&self, pool_ids: HashSet<B256>, block: Block) -> Result<Vec<Pool>> {
        let pool_infos = self.updater.lock().await.store().pools_by_id(&pool_ids);
        let pool_futures = pool_infos
            .into_iter()
            .map(|pool_info| self.fetcher.fetch_pool(&pool_info, block.into()))
            .collect::<Vec<_>>();

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

    fn name(&self) -> &str {
        "BalancerPoolFetcher"
    }
}

fn base_pool_factory(
    contract_instance: &BalancerFactoryInstance,
) -> BalancerV2BasePoolFactory::Instance {
    BalancerV2BasePoolFactory::Instance::new(
        *contract_instance.address(),
        contract_instance.provider().clone(),
    )
}

fn collect_pool_results(pools: Vec<Result<PoolStatus>>) -> Result<Vec<Pool>> {
    pools
        .into_iter()
        .filter_map(|pool| match pool {
            Ok(pool) => Some(Ok(pool.active()?)),
            // Error issued by the contract alloy contract calls
            Err(err) => match err.downcast_ref::<alloy::contract::Error>() {
                Some(err) if err.is_contract_error() => None,
                _ => Some(Err(err)),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::sources::balancer_v2::{
            pools::{PoolKind, weighted},
            swap::fixed_point::Bfp,
        },
        ethrpc::alloy::errors::{testing_alloy_contract_error, testing_alloy_node_error},
    };

    #[tokio::test]
    async fn collecting_results_filters_paused_pools_and_contract_errors() {
        let results = vec![
            Ok(PoolStatus::Active(Pool {
                id: Default::default(),
                kind: PoolKind::Weighted(weighted::PoolState {
                    tokens: Default::default(),
                    swap_fee: Bfp::zero(),
                    version: Default::default(),
                }),
            })),
            Ok(PoolStatus::Paused),
            Err(testing_alloy_contract_error().into()),
        ];
        assert_eq!(collect_pool_results(results).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn collecting_results_forwards_node_error() {
        let node_err = Err(testing_alloy_node_error().into());
        assert!(collect_pool_results(vec![node_err]).is_err());
    }
}
