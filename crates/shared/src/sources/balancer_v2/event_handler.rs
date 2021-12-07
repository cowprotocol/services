//! This event handler contains mostly boiler plate code for the implementation of `EventRetrieving`
//! and `EventStoring` for Balancer Pool Factory contracts and `PoolStorage` respectively.
//! Because there are multiple factory contracts for which we rely on event data, the
//! `BalancerPoolRegistry` is responsible for multiple EventHandlers.
//!
//! Apart from the event handling boiler plate, there are a few helper methods used as adapters
//! for converting received contract event data into appropriate internal structs to be passed
//! along to the `PoolStorage` (database) for update
//!
//! Due to limitations of `EventRetrieving` we must put each event handler behind its own Mutex.
//! - These mutexes are locked during synchronization and pool fetching.
//!
//! *Note that* when loading pool from a cold start synchronization can take quite long, but is
//! otherwise as quick as possible (i.e. taking advantage of as much cached information as possible).

use super::{
    pool_init::PoolInitializing,
    pool_storage::{PoolStorage, RegisteredStablePool, RegisteredWeightedPool},
    pools::{common, FactoryIndexing},
};
use crate::{
    event_handling::{BlockNumber, EventHandler, EventStoring},
    impl_event_retrieving,
    maintenance::Maintaining,
    Web3,
};
use anyhow::{anyhow, Result};
use contracts::{
    balancer_v2_base_pool_factory::{self, Event as BasePoolFactoryEvent},
    BalancerV2BasePoolFactory, BalancerV2StablePoolFactory, BalancerV2WeightedPool2TokensFactory,
    BalancerV2WeightedPoolFactory,
};
use ethcontract::{Event as EthContractEvent, H256};
use model::TokenPair;
use std::{collections::HashSet, ops::RangeInclusive, sync::Arc};
use tokio::sync::Mutex;

/// Type alias for the internal event updater type.
type PoolUpdater<Factory> =
    Mutex<EventHandler<Web3, BasePoolFactoryContract, PoolStorage<Factory>>>;

/// The Pool Registry maintains an event handler for each of the Balancer Pool Factory contracts
/// and maintains a `PoolStorage` for each.
/// Pools are read from this registry, via the public method `pool_ids_for_token_pairs`
/// which takes a collection of `TokenPair`, gets the relevant pools from each `PoolStorage`
/// and returns a merged de-duplicated version of the results.
pub struct BalancerPoolRegistry {
    weighted_pool_updater: PoolUpdater<BalancerV2WeightedPoolFactory>,
    two_token_pool_updater: PoolUpdater<BalancerV2WeightedPool2TokensFactory>,
    stable_pool_updater: PoolUpdater<BalancerV2StablePoolFactory>,
}

impl BalancerPoolRegistry {
    /// Deployed Pool Factories are loaded internally from the provided `web3` which is also used
    /// together with `token_info_fetcher` to construct a `PoolInfoFetcher` for each Event Handler.
    pub async fn new(
        web3: Web3,
        pool_initializer: impl PoolInitializing,
        common_pool_fetcher: Arc<dyn common::PoolInfoFetching>,
    ) -> Result<Self> {
        let weighted_pool_factory = BalancerV2WeightedPoolFactory::deployed(&web3).await?;
        let two_token_pool_factory = BalancerV2WeightedPool2TokensFactory::deployed(&web3).await?;
        let stable_pool_factory = BalancerV2StablePoolFactory::deployed(&web3).await?;

        let initial_pools = pool_initializer.initialize_pools().await?;

        macro_rules! create_pool_updater {
            ($factory:expr, $initial_pools:expr) => {{
                let factory = $factory;
                Mutex::new(EventHandler::new(
                    web3.clone(),
                    BasePoolFactoryContract(BalancerV2BasePoolFactory::with_deployment_info(
                        &web3,
                        factory.address(),
                        factory.deployment_information(),
                    )),
                    PoolStorage::new(factory, $initial_pools, common_pool_fetcher.clone()),
                    Some(initial_pools.fetched_block_number),
                ))
            }};
        }

        let weighted_pool_updater =
            create_pool_updater!(weighted_pool_factory, initial_pools.weighted_pools);
        let two_token_pool_updater =
            create_pool_updater!(two_token_pool_factory, initial_pools.weighted_2token_pools);
        let stable_pool_updater =
            create_pool_updater!(stable_pool_factory, initial_pools.stable_pools);

        Ok(Self {
            weighted_pool_updater,
            two_token_pool_updater,
            stable_pool_updater,
        })
    }

    /// Retrieves Registered Pools from each Pool Store in the Registry and
    /// returns the combined pool ids.
    /// Primarily intended to be used by `BalancerPoolFetcher`.
    pub async fn pool_ids_for_token_pairs(
        &self,
        token_pairs: &HashSet<TokenPair>,
    ) -> HashSet<H256> {
        let mut pool_ids = HashSet::new();
        pool_ids.extend(
            self.weighted_pool_updater
                .lock()
                .await
                .store
                .pool_ids_for_token_pairs(token_pairs),
        );
        pool_ids.extend(
            self.two_token_pool_updater
                .lock()
                .await
                .store
                .pool_ids_for_token_pairs(token_pairs),
        );
        pool_ids.extend(
            self.stable_pool_updater
                .lock()
                .await
                .store
                .pool_ids_for_token_pairs(token_pairs),
        );
        pool_ids
    }

    pub async fn get_weighted_pools(
        &self,
        pool_ids: &HashSet<H256>,
    ) -> Vec<RegisteredWeightedPool> {
        let mut pools: Vec<RegisteredWeightedPool> = Vec::new();
        pools.extend(
            self.weighted_pool_updater
                .lock()
                .await
                .store
                .pools_by_id(pool_ids),
        );
        pools.extend(
            self.two_token_pool_updater
                .lock()
                .await
                .store
                .pools_by_id(pool_ids),
        );
        pools
    }

    pub async fn get_stable_pools(&self, pool_ids: &HashSet<H256>) -> Vec<RegisteredStablePool> {
        self.stable_pool_updater
            .lock()
            .await
            .store
            .pools_by_id(pool_ids)
    }
}

#[async_trait::async_trait]
impl<Factory> EventStoring<BasePoolFactoryEvent> for PoolStorage<Factory>
where
    Factory: FactoryIndexing,
{
    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<BasePoolFactoryEvent>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()> {
        tracing::debug!("replacing {} events for block {:?}", events.len(), range);

        self.remove_pools_newer_than_block(range.start().to_u64());
        self.append_events(events).await
    }

    async fn append_events(
        &mut self,
        events: Vec<EthContractEvent<BasePoolFactoryEvent>>,
    ) -> Result<()> {
        tracing::debug!("inserting {} events", events.len());

        for event in events {
            let block_created = event
                .meta
                .ok_or_else(|| anyhow!("event missing metadata"))?
                .block_number;
            let BasePoolFactoryEvent::PoolCreated(pool_created) = event.data;

            self.index_pool_creation(pool_created, block_created)
                .await?;
        }

        Ok(())
    }

    async fn last_event_block(&self) -> Result<u64> {
        Ok(self.last_event_block())
    }
}

impl_event_retrieving! {
    pub BasePoolFactoryContract for balancer_v2_base_pool_factory
}

#[async_trait::async_trait]
impl Maintaining for BalancerPoolRegistry {
    async fn run_maintenance(&self) -> Result<()> {
        futures::try_join!(
            self.two_token_pool_updater.run_maintenance(),
            self.weighted_pool_updater.run_maintenance(),
            self.stable_pool_updater.run_maintenance(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sources::balancer_v2::{
            pool_init::{EmptyPoolInitializer, SubgraphPoolInitializer},
            pools::common::PoolInfoFetcher,
        },
        token_info::TokenInfoFetcher,
        transport,
    };
    use contracts::BalancerV2Vault;
    use reqwest::Client;

    #[tokio::test]
    #[ignore]
    async fn balancer_indexed_pool_events_match_subgraph() {
        let transport = transport::create_env_test_transport();
        let web3 = Web3::new(transport);
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();

        println!("Indexing events for chain {}", chain_id);
        crate::tracing::initialize_for_tests("warn,shared=debug");

        let pool_init = EmptyPoolInitializer::for_chain(chain_id);
        let token_infos = TokenInfoFetcher { web3: web3.clone() };
        let pool_info = PoolInfoFetcher::new(
            BalancerV2Vault::deployed(&web3).await.unwrap(),
            Arc::new(token_infos),
        );
        let registry = BalancerPoolRegistry::new(web3, pool_init, Arc::new(pool_info))
            .await
            .unwrap();

        // index all the pools.
        registry.run_maintenance().await.unwrap();

        // compare to what the subgraph says.
        let client = SubgraphPoolInitializer::new(chain_id, Client::new()).unwrap();
        let subgraph_pools = client.initialize_pools().await.unwrap();

        macro_rules! assert_all_subgraph_pools_indexed {
            ($subgraph_pools:expr, $pool_updater:expr) => {{
                let pool_updater = $pool_updater.lock().await;
                for mut subgraph_pool in $subgraph_pools {
                    let indexed_pool = pool_updater
                        .store
                        .pool_by_id(subgraph_pool.common.id)
                        .unwrap_or_else(|| panic!("pool {:?} did not get indexed", subgraph_pool.common.id));

                    // Subgraph pools don't correctly set the created block, so
                    // fix it here so we can compare the other fields in the
                    // following assert.
                    subgraph_pool.common.block_created = indexed_pool.common.block_created;
                    assert_eq!(indexed_pool, &subgraph_pool);

                    tracing::info!(pool = ?indexed_pool);
                }
            }};
        }

        assert_all_subgraph_pools_indexed!(
            subgraph_pools.weighted_pools,
            &registry.weighted_pool_updater
        );

        assert_all_subgraph_pools_indexed!(
            subgraph_pools.weighted_2token_pools,
            &registry.two_token_pool_updater
        );

        assert_all_subgraph_pools_indexed!(
            subgraph_pools.stable_pools,
            &registry.stable_pool_updater
        );
    }
}
