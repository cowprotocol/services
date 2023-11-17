//! A pool registry for a single pool factory that is generic on its type of
//! pool.

use {
    super::{internal::InternalPoolFetching, pool_storage::PoolStorage},
    crate::{
        ethcontract_error::EthcontractErrorType,
        event_handling::{EventHandler, EventRetrieving},
        maintenance::Maintaining,
        recent_block_cache::Block,
        sources::balancer_v2::pools::{
            common::PoolInfoFetching,
            FactoryIndexing,
            Pool,
            PoolIndexing,
            PoolStatus,
        },
    },
    anyhow::Result,
    contracts::{balancer_v2_base_pool_factory, BalancerV2BasePoolFactory},
    ethcontract::{dyns::DynAllEventsBuilder, errors::MethodError, BlockId, Instance, H256},
    ethrpc::{
        current_block::{BlockNumberHash, BlockRetrieving},
        Web3,
        Web3CallBatch,
        Web3Transport,
        MAX_BATCH_SIZE,
    },
    futures::{future, FutureExt},
    hex_literal::hex,
    model::TokenPair,
    std::{
        collections::HashSet,
        sync::{Arc, RwLock},
    },
    tokio::sync::Mutex,
};

pub struct BasePoolFactoryContract(BalancerV2BasePoolFactory);

const POOL_CREATED_TOPIC: H256 = H256(hex!(
    "83a48fbcfc991335314e74d0496aab6a1987e992ddc85dddbcc4d6dd6ef2e9fc"
));

impl EventRetrieving for BasePoolFactoryContract {
    type Event = balancer_v2_base_pool_factory::Event;

    fn get_events(&self) -> DynAllEventsBuilder<Self::Event> {
        let mut events = self.0.all_events();
        events.filter = events.filter.topic0(POOL_CREATED_TOPIC.into());
        events
    }
}

/// Type alias for the internal event updater type.
type PoolUpdater<Factory> = Mutex<EventHandler<BasePoolFactoryContract, PoolStorage<Factory>>>;

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
    web3: Web3,
    fetcher: Arc<dyn PoolInfoFetching<Factory>>,
    updater: PoolUpdater<Factory>,
    non_existent_pools: RwLock<HashSet<H256>>,
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
            non_existent_pools: Default::default(),
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

    async fn pools_by_id(&self, mut pool_ids: HashSet<H256>, block: Block) -> Result<Vec<Pool>> {
        {
            let non_existent_pools = self.non_existent_pools.read().unwrap();
            pool_ids.retain(|id| !non_existent_pools.contains(id));
        }
        let mut batch = Web3CallBatch::new(self.web3.transport().clone());
        let block = BlockId::Number(block.into());

        let pool_infos = self.updater.lock().await.store().pools_by_id(&pool_ids);
        let pool_futures = pool_infos
            .into_iter()
            .map(|pool_info| {
                let id = pool_info.common().id;
                self.fetcher
                    .fetch_pool(&pool_info, &mut batch, block)
                    .map(move |result| (id, result))
            })
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;

        let results = future::join_all(pool_futures).await;
        let (pools, missing_ids) = collect_pool_results(results)?;
        if !missing_ids.is_empty() {
            self.non_existent_pools.write().unwrap().extend(missing_ids);
        }
        Ok(pools)
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

fn base_pool_factory(contract_instance: &Instance<Web3Transport>) -> BalancerV2BasePoolFactory {
    BalancerV2BasePoolFactory::with_deployment_info(
        &contract_instance.web3(),
        contract_instance.address(),
        contract_instance.deployment_information(),
    )
}

/// Returns the list of found pools and a list of pool ids that could not be
/// found.
fn collect_pool_results(
    results: Vec<(H256, Result<PoolStatus>)>,
) -> Result<(Vec<Pool>, Vec<H256>)> {
    let mut fetched_pools = Vec::with_capacity(results.len());
    let mut missing_ids = vec![];
    for (id, result) in results {
        match result {
            Ok(PoolStatus::Active(pool)) => fetched_pools.push(pool),
            Ok(PoolStatus::Disabled) => missing_ids.push(id),
            Ok(PoolStatus::Paused) => {}
            Err(err) if is_contract_error(&err) => missing_ids.push(id),
            Err(err) => return Err(err),
        }
    }
    Ok((fetched_pools, missing_ids))
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
    use {
        super::*,
        crate::{
            ethcontract_error,
            sources::balancer_v2::{
                pools::{weighted, PoolKind},
                swap::fixed_point::Bfp,
            },
        },
        std::str::FromStr,
    };

    #[tokio::test]
    async fn collecting_results_filters_paused_pools_and_contract_errors() {
        let bad_pool =
            H256::from_str("e337fcd52afd6b98847baab279cda6c3980fcb185da9e959fd489ffd210eac60")
                .unwrap();
        let results = vec![
            (
                Default::default(),
                Ok(PoolStatus::Active(Pool {
                    id: Default::default(),
                    kind: PoolKind::Weighted(weighted::PoolState {
                        tokens: Default::default(),
                        swap_fee: Bfp::zero(),
                        version: Default::default(),
                    }),
                })),
            ),
            (Default::default(), Ok(PoolStatus::Paused)),
            (
                bad_pool,
                Err(ethcontract_error::testing_contract_error().into()),
            ),
        ];
        let (fetched, missing) = collect_pool_results(results).unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(missing, vec![bad_pool]);
    }

    #[tokio::test]
    async fn collecting_results_forwards_node_error() {
        let node_err = (
            Default::default(),
            Err(ethcontract_error::testing_node_error().into()),
        );
        let result = collect_pool_results(vec![node_err]);
        assert!(result.is_err());
    }
}
