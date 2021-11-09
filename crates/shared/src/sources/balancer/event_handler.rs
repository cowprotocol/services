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
use crate::{
    event_handling::{BlockNumber, EventHandler, EventIndex, EventStoring},
    impl_event_retrieving,
    maintenance::Maintaining,
    sources::balancer::{
        info_fetching::PoolInfoFetching,
        pool_init::PoolInitializing,
        pool_storage::{
            PoolCreated, PoolStorage, PoolType, RegisteredStablePool, RegisteredWeightedPool,
        },
    },
    Web3,
};
use anyhow::{anyhow, Context, Result};
use contracts::{
    balancer_v2_stable_pool_factory::{self, Event as StablePoolFactoryEvent},
    balancer_v2_weighted_pool_2_tokens_factory::{self, Event as WeightedPool2TokensFactoryEvent},
    balancer_v2_weighted_pool_factory::{self, Event as WeightedPoolFactoryEvent},
    BalancerV2StablePoolFactory, BalancerV2WeightedPool2TokensFactory,
    BalancerV2WeightedPoolFactory,
};
use ethcontract::{Event as EthContractEvent, H256};
use model::TokenPair;
use std::{collections::HashSet, ops::RangeInclusive, sync::Arc};
use tokio::sync::Mutex;

/// The Pool Registry maintains an event handler for each of the Balancer Pool Factory contracts
/// and maintains a `PoolStorage` for each.
/// Pools are read from this registry, via the public method `get_pool_ids_containing_token_pairs`
/// which takes a collection of `TokenPair`, gets the relevant pools from each `PoolStorage`
/// and returns a merged de-duplicated version of the results.
pub struct BalancerPoolRegistry {
    weighted_pool_updater:
        Mutex<EventHandler<Web3, BalancerV2WeightedPoolFactoryContract, PoolStorage>>,
    two_token_pool_updater:
        Mutex<EventHandler<Web3, BalancerV2WeightedPool2TokensFactoryContract, PoolStorage>>,
    stable_pool_updater:
        Mutex<EventHandler<Web3, BalancerV2StablePoolFactoryContract, PoolStorage>>,
}

impl BalancerPoolRegistry {
    /// Deployed Pool Factories are loaded internally from the provided `web3` which is also used
    /// together with `token_info_fetcher` to construct a `PoolInfoFetcher` for each Event Handler.
    pub async fn new(
        web3: Web3,
        pool_initializer: impl PoolInitializing,
        pool_info: Arc<dyn PoolInfoFetching>,
    ) -> Result<Self> {
        let weighted_pool_factory = BalancerV2WeightedPoolFactory::deployed(&web3).await?;
        let two_token_pool_factory = BalancerV2WeightedPool2TokensFactory::deployed(&web3).await?;
        let stable_pool_factory = BalancerV2StablePoolFactory::deployed(&web3).await?;

        let initial_pools = pool_initializer.initialize_pools().await?;

        let weighted_pool_updater = Mutex::new(EventHandler::new(
            web3.clone(),
            BalancerV2WeightedPoolFactoryContract(weighted_pool_factory),
            PoolStorage::new(initial_pools.weighted_pools, vec![], pool_info.clone()),
            Some(initial_pools.fetched_block_number),
        ));
        let two_token_pool_updater = Mutex::new(EventHandler::new(
            web3.clone(),
            BalancerV2WeightedPool2TokensFactoryContract(two_token_pool_factory),
            PoolStorage::new(
                initial_pools.weighted_2token_pools,
                vec![],
                pool_info.clone(),
            ),
            Some(initial_pools.fetched_block_number),
        ));

        let stable_pool_updater = Mutex::new(EventHandler::new(
            web3.clone(),
            BalancerV2StablePoolFactoryContract(stable_pool_factory),
            PoolStorage::new(vec![], initial_pools.stable_pools, pool_info),
            Some(initial_pools.fetched_block_number),
        ));

        Ok(Self {
            weighted_pool_updater,
            two_token_pool_updater,
            stable_pool_updater,
        })
    }

    /// Retrieves Registered Pools from each Pool Store in the Registry and
    /// returns the combined pool ids.
    /// Primarily intended to be used by `BalancerPoolFetcher`.
    pub async fn get_pool_ids_containing_token_pairs(
        &self,
        token_pairs: HashSet<TokenPair>,
    ) -> HashSet<H256> {
        let mut pool_ids = HashSet::new();
        pool_ids.extend(
            self.weighted_pool_updater
                .lock()
                .await
                .store
                .ids_for_pools_containing_token_pairs(token_pairs.clone()),
        );
        pool_ids.extend(
            self.two_token_pool_updater
                .lock()
                .await
                .store
                .ids_for_pools_containing_token_pairs(token_pairs.clone()),
        );
        pool_ids.extend(
            self.stable_pool_updater
                .lock()
                .await
                .store
                .ids_for_pools_containing_token_pairs(token_pairs),
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
                .weighted_pools_for(pool_ids),
        );
        pools.extend(
            self.two_token_pool_updater
                .lock()
                .await
                .store
                .weighted_pools_for(pool_ids),
        );
        pools
    }

    pub async fn get_stable_pools(&self, pool_ids: &HashSet<H256>) -> Vec<RegisteredStablePool> {
        self.stable_pool_updater
            .lock()
            .await
            .store
            .stable_pools_for(pool_ids)
    }
}

#[async_trait::async_trait]
impl EventStoring<WeightedPoolFactoryEvent> for PoolStorage {
    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<WeightedPoolFactoryEvent>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()> {
        self.replace_events_inner(
            range.start().to_u64(),
            convert_weighted_pool_created(events)?,
        )
        .await
    }

    async fn append_events(
        &mut self,
        events: Vec<EthContractEvent<WeightedPoolFactoryEvent>>,
    ) -> Result<()> {
        tracing::debug!(
            "inserting {} Balancer Weighted Pools from events",
            events.len()
        );
        self.insert_events(convert_weighted_pool_created(events)?)
            .await
    }

    async fn last_event_block(&self) -> Result<u64> {
        Ok(self.last_event_block())
    }
}

#[async_trait::async_trait]
impl EventStoring<WeightedPool2TokensFactoryEvent> for PoolStorage {
    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<WeightedPool2TokensFactoryEvent>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()> {
        self.replace_events_inner(
            range.start().to_u64(),
            convert_two_token_pool_created(events)?,
        )
        .await
    }

    async fn append_events(
        &mut self,
        events: Vec<EthContractEvent<WeightedPool2TokensFactoryEvent>>,
    ) -> Result<()> {
        tracing::debug!(
            "Inserting {} Balancer Weighted 2-Token Pools from events",
            events.len()
        );
        self.insert_events(convert_two_token_pool_created(events)?)
            .await
    }

    async fn last_event_block(&self) -> Result<u64> {
        Ok(self.last_event_block())
    }
}

#[async_trait::async_trait]
impl EventStoring<StablePoolFactoryEvent> for PoolStorage {
    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<StablePoolFactoryEvent>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()> {
        self.replace_events_inner(range.start().to_u64(), convert_stable_pool_created(events)?)
            .await
    }

    async fn append_events(
        &mut self,
        events: Vec<EthContractEvent<StablePoolFactoryEvent>>,
    ) -> Result<()> {
        tracing::debug!(
            "Inserting {} Balancer Stable Pools from events",
            events.len()
        );
        self.insert_events(convert_stable_pool_created(events)?)
            .await
    }

    async fn last_event_block(&self) -> Result<u64> {
        Ok(self.last_event_block())
    }
}

impl_event_retrieving! {
    pub BalancerV2WeightedPoolFactoryContract for balancer_v2_weighted_pool_factory
}

impl_event_retrieving! {
    pub BalancerV2WeightedPool2TokensFactoryContract for balancer_v2_weighted_pool_2_tokens_factory
}

impl_event_retrieving! {
    pub BalancerV2StablePoolFactoryContract for balancer_v2_stable_pool_factory
}

#[async_trait::async_trait]
impl Maintaining for BalancerPoolRegistry {
    async fn run_maintenance(&self) -> Result<()> {
        futures::try_join!(
            self.two_token_pool_updater.run_maintenance(),
            self.weighted_pool_updater.run_maintenance(),
        )?;
        Ok(())
    }
}

/// Adapter methods for converting contract events from each pool factory into a single
/// `PoolCreated` struct that all event handlers are compatible with.
fn contract_to_pool_creation<T>(
    contract_events: Vec<EthContractEvent<T>>,
    adapter: impl Fn(T) -> PoolCreated,
) -> Result<Vec<(EventIndex, PoolCreated)>> {
    contract_events
        .into_iter()
        .map(|EthContractEvent { data, meta }| {
            let meta = meta.ok_or_else(|| anyhow!("event without metadata"))?;
            Ok((EventIndex::from(&meta), adapter(data)))
        })
        .collect::<Result<Vec<_>>>()
        .context("failed to convert events")
}

fn convert_weighted_pool_created(
    events: Vec<EthContractEvent<WeightedPoolFactoryEvent>>,
) -> Result<Vec<(EventIndex, PoolCreated)>> {
    contract_to_pool_creation(events, |event| match event {
        WeightedPoolFactoryEvent::PoolCreated(creation) => PoolCreated {
            pool_type: PoolType::Weighted,
            pool_address: creation.pool,
        },
    })
}

fn convert_two_token_pool_created(
    events: Vec<EthContractEvent<WeightedPool2TokensFactoryEvent>>,
) -> Result<Vec<(EventIndex, PoolCreated)>> {
    contract_to_pool_creation(events, |event| match event {
        WeightedPool2TokensFactoryEvent::PoolCreated(creation) => PoolCreated {
            pool_type: PoolType::Weighted,
            pool_address: creation.pool,
        },
    })
}

fn convert_stable_pool_created(
    events: Vec<EthContractEvent<StablePoolFactoryEvent>>,
) -> Result<Vec<(EventIndex, PoolCreated)>> {
    contract_to_pool_creation(events, |event| match event {
        StablePoolFactoryEvent::PoolCreated(creation) => PoolCreated {
            pool_type: PoolType::Stable,
            pool_address: creation.pool,
        },
    })
}
