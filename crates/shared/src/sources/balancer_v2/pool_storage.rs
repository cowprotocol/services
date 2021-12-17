//! Pool Storage contains all the essential models required by the balancer module for operating
//! between different knowledge-levels of pool information.
//!
//! To briefly list and describe each of the models.
//!
//! 1. `PoolCreated`:
//!     contains only the `pool_address` as this is the only information known about the pool
//!     at the time of event emission from the pool's factory contract.
//!
//! 2. `RegisteredWeightedPool` & `RegisteredStablePool`:
//!     contains all constant/static information about the pool (that which is not block-sensitive).
//!     That is,
//!     `pool_id`, `address`, `tokens`, `scaling_exponents`, `block_created` (i.e. `CommonPoolData`)
//!     and `normalized_weights` (specific to weighted pools).
//!     When the `PoolCreated` event is received by the event handler, an instance of this type is
//!     constructed by fetching all additional information about the pool via `PoolInfoFetching`.
//!
//!     It is these pools which are stored, in memory, as part of the `BalancerPoolRegistry`.
//!
//! 3. `PoolStorage`:
//!     This should be thought of as the Pool Registry's database which stores all static pool
//!     information in data structures that provide efficient lookup for the `PoolFetcher`.
//!
//!     Pool Storage implements all the CRUD methods expected of such a database.
//!
//! Tests included here are those pertaining to the expected functionality of `PoolStorage`

use crate::event_handling::{BlockNumber, EventStoring};

use super::pools::{common, FactoryIndexing, PoolIndexing};
use anyhow::{anyhow, Result};
use contracts::balancer_v2_base_pool_factory::{
    event_data::PoolCreated, Event as BasePoolFactoryEvent,
};
use ethcontract::{Event, H160, H256};
use model::TokenPair;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    ops::RangeInclusive,
    sync::Arc,
};

#[cfg(test)]
pub type CommonPoolData = common::PoolInfo;

#[cfg(test)]
pub fn common_pool(seed: u8) -> CommonPoolData {
    CommonPoolData {
        id: H256([seed; 32]),
        address: H160([seed; 20]),
        tokens: vec![H160([seed; 20]), H160([seed + 1; 20])],
        scaling_exponents: vec![0, 0],
        block_created: seed as _,
    }
}

pub type RegisteredWeightedPool = super::pools::weighted::PoolInfo;

pub type RegisteredStablePool = super::pools::stable::PoolInfo;

/// PoolStorage represents in-memory storage of all deployed Balancer Pools
pub struct PoolStorage<Factory>
where
    Factory: FactoryIndexing,
{
    /// Component used to fetch pool information.
    pool_info_fetcher: Arc<dyn common::PoolInfoFetching<Factory>>,
    /// Used for O(1) access to all pool_ids for a given token
    pools_by_token: HashMap<H160, HashSet<H256>>,
    /// All indexed pool infos by ID.
    pools: HashMap<H256, Factory::PoolInfo>,
    /// The block the initial pools were fetched on. This block is considered
    /// reorg-safe and events prior to this block do not get replaced.
    initial_fetched_block: u64,
}

impl<Factory> PoolStorage<Factory>
where
    Factory: FactoryIndexing,
{
    pub fn new(
        initial_pools: Vec<Factory::PoolInfo>,
        pool_info_fetcher: Arc<dyn common::PoolInfoFetching<Factory>>,
    ) -> Self {
        initial_pools.into_iter().fold(
            Self {
                pool_info_fetcher,
                pools_by_token: Default::default(),
                pools: Default::default(),
                initial_fetched_block: 0,
            },
            |mut storage, pool| {
                storage.initial_fetched_block =
                    cmp::max(storage.initial_fetched_block, pool.common().block_created);
                storage.insert_pool(pool);

                storage
            },
        )
    }

    /// Returns all pools containing both tokens from `TokenPair`
    fn pool_ids_for_token_pair(&self, token_pair: &TokenPair) -> impl Iterator<Item = H256> + '_ {
        let (token0, token1) = token_pair.get();

        let pools0 = self.pools_by_token.get(&token0);
        let pools1 = self.pools_by_token.get(&token1);

        pools0
            .zip(pools1)
            .into_iter()
            .flat_map(|(pools0, pools1)| pools0.intersection(pools1))
            .copied()
    }

    /// Given a collection of `TokenPair`, returns all pools containing at least
    /// one of the pairs.
    pub fn pool_ids_for_token_pairs(&self, token_pairs: &HashSet<TokenPair>) -> HashSet<H256> {
        token_pairs
            .iter()
            .flat_map(|pair| self.pool_ids_for_token_pair(pair))
            .collect()
    }

    /// Returns a pool by ID or none if no such pool exists.
    pub fn pool_by_id(&self, pool_id: H256) -> Option<&Factory::PoolInfo> {
        self.pools.get(&pool_id)
    }

    /// Returns all pool infos by their IDs.
    pub fn pools_by_id(&self, pool_ids: &HashSet<H256>) -> Vec<Factory::PoolInfo> {
        pool_ids
            .iter()
            .filter_map(|pool_id| self.pool_by_id(*pool_id))
            .cloned()
            .collect()
    }

    fn insert_pool(&mut self, pool: Factory::PoolInfo) {
        for token in &pool.common().tokens {
            self.pools_by_token
                .entry(*token)
                .or_default()
                .insert(pool.common().id);
        }
        self.pools.insert(pool.common().id, pool);
    }

    /// Indexes a new pool creation event.
    pub async fn index_pool_creation(
        &mut self,
        pool_creation: PoolCreated,
        block_created: u64,
    ) -> Result<()> {
        let pool = self
            .pool_info_fetcher
            .fetch_pool_info(pool_creation.pool, block_created)
            .await?;
        self.insert_pool(pool);

        Ok(())
    }

    /// Removes all pool creations from the specified block.
    pub fn remove_pools_newer_than_block(&mut self, delete_from_block_number: u64) {
        let block = if delete_from_block_number <= self.initial_fetched_block {
            tracing::debug!(
                "skipping deleting events from {}..={}",
                delete_from_block_number,
                self.initial_fetched_block,
            );
            self.initial_fetched_block + 1
        } else {
            delete_from_block_number
        };

        let num_pools = self.pools.len();
        self.pools
            .retain(|_, pool| pool.common().block_created < block);

        if num_pools == self.pools.len() {
            // We didnt' actually remove any pools, so no need to rebuild the
            // tokens to pools map.
            return;
        }

        // Note that this could result in an empty set for some tokens.
        for pool_set in self.pools_by_token.values_mut() {
            pool_set.retain(|pool_id| self.pools.contains_key(pool_id));
        }
    }

    pub fn last_event_block(&self) -> u64 {
        // Technically we could keep this updated more effectively in a field on balancer pools,
        // but the maintenance seems like more overhead that needs to be tested.
        self.pools
            .values()
            .map(|pool| pool.common().block_created)
            .max()
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl<Factory> EventStoring<BasePoolFactoryEvent> for PoolStorage<Factory>
where
    Factory: FactoryIndexing,
{
    async fn replace_events(
        &mut self,
        events: Vec<Event<BasePoolFactoryEvent>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()> {
        tracing::debug!("replacing {} events for block {:?}", events.len(), range);

        self.remove_pools_newer_than_block(range.start().to_u64());
        self.append_events(events).await
    }

    async fn append_events(&mut self, events: Vec<Event<BasePoolFactoryEvent>>) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::{
        pools::{common::MockPoolInfoFetching, MockFactoryIndexing},
        swap::fixed_point::Bfp,
    };
    use maplit::{hashmap, hashset};
    use mockall::predicate::eq;

    pub type PoolInitData = (
        Vec<H256>,
        Vec<H160>,
        Vec<H160>,
        Vec<Bfp>,
        Vec<(PoolCreated, u64)>,
    );
    fn pool_init_data(start: usize, end: usize) -> PoolInitData {
        let pool_ids: Vec<H256> = (start..=end)
            .map(|i| H256::from_low_u64_be(i as u64))
            .collect();
        let pool_addresses: Vec<H160> = (start..=end)
            .map(|i| H160::from_low_u64_be(i as u64))
            .collect();
        let tokens: Vec<H160> = (start..=end + 1)
            .map(|i| H160::from_low_u64_be(i as u64))
            .collect();
        let weights: Vec<Bfp> = (start..=end + 1).map(|i| Bfp::from_wei(i.into())).collect();
        let creation_events: Vec<(PoolCreated, u64)> = (start..=end)
            .map(|i| {
                (
                    PoolCreated {
                        pool: pool_addresses[i],
                    },
                    i as u64,
                )
            })
            .collect();

        (pool_ids, pool_addresses, tokens, weights, creation_events)
    }

    #[test]
    fn initialize_storage() {
        let storage = PoolStorage::new(
            vec![
                RegisteredWeightedPool {
                    common: CommonPoolData {
                        id: H256([1; 32]),
                        address: H160([1; 20]),
                        tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                        scaling_exponents: vec![0, 0],
                        block_created: 0,
                    },
                    weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                },
                RegisteredWeightedPool {
                    common: CommonPoolData {
                        id: H256([2; 32]),
                        address: H160([2; 20]),
                        tokens: vec![H160([0x11; 20]), H160([0x33; 20]), H160([0x77; 20])],
                        scaling_exponents: vec![0, 0],
                        block_created: 0,
                    },
                    weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                    ],
                },
                RegisteredWeightedPool {
                    common: CommonPoolData {
                        id: H256([3; 32]),
                        address: H160([3; 20]),
                        tokens: vec![H160([0x11; 20]), H160([0x77; 20])],
                        scaling_exponents: vec![0, 0],
                        block_created: 0,
                    },
                    weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                },
            ],
            Arc::new(MockPoolInfoFetching::<MockFactoryIndexing>::new()),
        );

        assert_eq!(
            storage.pools_by_token,
            hashmap! {
                H160([0x11; 20]) => hashset![H256([1; 32]), H256([2; 32]), H256([3; 32])],
                H160([0x22; 20]) => hashset![H256([1; 32])],
                H160([0x33; 20]) => hashset![H256([2; 32])],
                H160([0x77; 20]) => hashset![H256([2; 32]), H256([3; 32])],
            }
        );
    }

    #[tokio::test]
    async fn insert_pool_events() {
        let n = 3usize;
        let (pool_ids, pool_addresses, tokens, weights, creation_events) = pool_init_data(0, n);

        let mut mock_pool_fetcher = MockPoolInfoFetching::<MockFactoryIndexing>::new();
        for i in 0..n {
            let expected_pool_data = RegisteredWeightedPool {
                common: CommonPoolData {
                    id: pool_ids[i],
                    address: pool_addresses[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    scaling_exponents: vec![0, 0],
                    block_created: creation_events[i].1,
                },
                weights: vec![weights[i], weights[i + 1]],
            };

            mock_pool_fetcher
                .expect_fetch_pool_info()
                .with(eq(pool_addresses[i]), eq(creation_events[i].1))
                .returning({
                    let expected_pool_data = expected_pool_data.clone();
                    move |_, _| Ok(expected_pool_data.clone())
                });
        }

        let mut pool_store = PoolStorage::new(Default::default(), Arc::new(mock_pool_fetcher));
        for (pool_created, block_created) in creation_events.into_iter().take(n) {
            pool_store
                .index_pool_creation(pool_created, block_created)
                .await
                .unwrap();
        }

        // Note that it is never expected that blocks for events will differ,
        // but in this test block_created for the pool is the first block it receives.
        assert_eq!(pool_store.last_event_block(), 2);
        assert_eq!(
            pool_store.pools_by_token.get(&tokens[0]).unwrap(),
            &hashset! { pool_ids[0] }
        );
        assert_eq!(
            pool_store.pools_by_token.get(&tokens[1]).unwrap(),
            &hashset! { pool_ids[0], pool_ids[1] }
        );
        assert_eq!(
            pool_store.pools_by_token.get(&tokens[2]).unwrap(),
            &hashset! { pool_ids[1], pool_ids[2] }
        );
        assert_eq!(
            pool_store.pools_by_token.get(&tokens[3]).unwrap(),
            &hashset! { pool_ids[2] }
        );

        for i in 0..n {
            assert_eq!(
                pool_store.pools.get(&pool_ids[i]).unwrap(),
                &RegisteredWeightedPool {
                    common: CommonPoolData {
                        id: pool_ids[i],
                        address: pool_addresses[i],
                        tokens: vec![tokens[i], tokens[i + 1]],
                        scaling_exponents: vec![0, 0],
                        block_created: i as _,
                    },
                    weights: vec![weights[i], weights[i + 1]],
                },
            );
        }
    }

    #[tokio::test]
    async fn replace_pool_events() {
        let start_block = 0;
        let end_block = 5;
        let (pool_ids, pool_addresses, tokens, weights, creation_events) =
            pool_init_data(start_block, end_block);
        // Setup all the variables to initialize Balancer Pool State

        let mut mock_pool_fetcher = MockPoolInfoFetching::<MockFactoryIndexing>::new();
        for i in start_block..=end_block {
            let expected_pool_data = RegisteredWeightedPool {
                common: CommonPoolData {
                    id: pool_ids[i],
                    address: pool_addresses[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    scaling_exponents: vec![0, 0],
                    block_created: creation_events[i].1,
                },
                weights: vec![weights[i], weights[i + 1]],
            };

            mock_pool_fetcher
                .expect_fetch_pool_info()
                .with(eq(pool_addresses[i]), eq(creation_events[i].1))
                .returning({
                    let expected_pool_data = expected_pool_data.clone();
                    move |_, _| Ok(expected_pool_data.clone())
                });
        }

        let new_pool = RegisteredWeightedPool {
            common: CommonPoolData {
                id: H256::from_low_u64_be(43110),
                address: H160::from_low_u64_be(42),
                tokens: vec![H160::from_low_u64_be(808)],
                scaling_exponents: vec![0],
                block_created: 3,
            },
            weights: vec![Bfp::from_wei(1337.into())],
        };
        let new_creation = PoolCreated {
            pool: new_pool.common.address,
        };

        mock_pool_fetcher
            .expect_fetch_pool_info()
            .with(
                eq(new_pool.common.address),
                eq(new_pool.common.block_created),
            )
            .returning({
                let new_pool = new_pool.clone();
                move |_, _| Ok(new_pool.clone())
            });

        // Let the tests begin!
        let mut pool_store = PoolStorage::new(Default::default(), Arc::new(mock_pool_fetcher));
        for (pool_creation, block_created) in creation_events {
            pool_store
                .index_pool_creation(pool_creation, block_created)
                .await
                .unwrap();
        }

        // Make sure that we indexed all the initial events, and replace
        assert_eq!(pool_store.last_event_block(), end_block as u64);
        pool_store.remove_pools_newer_than_block(3);
        pool_store
            .index_pool_creation(new_creation, new_pool.common.block_created)
            .await
            .unwrap();

        // Everything until block 3 is unchanged.
        for i in 0..3 {
            assert_eq!(
                pool_store.pools.get(&pool_ids[i]).unwrap(),
                &RegisteredWeightedPool {
                    common: CommonPoolData {
                        id: pool_ids[i],
                        address: pool_addresses[i],
                        tokens: vec![tokens[i], tokens[i + 1]],
                        scaling_exponents: vec![0, 0],
                        block_created: i as u64,
                    },
                    weights: vec![weights[i], weights[i + 1]],
                },
            );
        }

        assert_eq!(
            pool_store.pools_by_token.get(&tokens[0]).unwrap(),
            &hashset! { pool_ids[0] }
        );
        assert_eq!(
            pool_store.pools_by_token.get(&tokens[1]).unwrap(),
            &hashset! { pool_ids[0], pool_ids[1] }
        );
        assert_eq!(
            pool_store.pools_by_token.get(&tokens[2]).unwrap(),
            &hashset! { pool_ids[1], pool_ids[2] }
        );
        assert_eq!(
            pool_store.pools_by_token.get(&tokens[3]).unwrap(),
            &hashset! { pool_ids[2] }
        );

        // Everything old from block 3 on is gone.
        for pool_id in pool_ids.iter().take(6).skip(3) {
            assert!(pool_store.pools.get(pool_id).is_none());
        }
        for token in tokens.iter().take(7).skip(4) {
            assert!(pool_store.pools_by_token.get(token).unwrap().is_empty());
        }

        // All new data is included.
        assert_eq!(
            pool_store.pools.get(&new_pool.common.id).unwrap(),
            &new_pool,
        );

        assert!(pool_store
            .pools_by_token
            .get(&new_pool.common.tokens[0])
            .is_some());
        assert_eq!(pool_store.last_event_block(), new_pool.common.block_created);
    }

    #[test]
    fn ids_for_pools_containing_token_pairs() {
        let n = 3;
        let (pool_ids, pool_addresses, tokens, _, _) = pool_init_data(0, n);
        let token_pairs: Vec<TokenPair> = (0..n)
            .map(|i| TokenPair::new(tokens[i], tokens[(i + 1) % n]).unwrap())
            .collect();

        let mut registry = PoolStorage::new(
            Default::default(),
            Arc::new(MockPoolInfoFetching::<MockFactoryIndexing>::new()),
        );
        // Test the empty registry.
        for token_pair in &token_pairs {
            assert_eq!(registry.pool_ids_for_token_pair(token_pair).next(), None);
        }

        // Now test non-empty pool with standard form.
        let mut weighted_pools = Vec::new();
        for i in 0..n {
            weighted_pools.push(RegisteredWeightedPool {
                common: CommonPoolData {
                    id: pool_ids[i],
                    tokens: tokens[i..n].to_owned(),
                    scaling_exponents: vec![],
                    block_created: 0,
                    address: pool_addresses[i],
                },
                weights: vec![],
            });
            registry.insert_pool(weighted_pools[i].clone());
        }
        // When n = 3, this above generates
        // pool_store.pools_by_token = hashmap! {
        //     tokens[0] => hashset! { pool_ids[0] },
        //     tokens[1] => hashset! { pool_ids[0], pool_ids[1]},
        //     tokens[2] => hashset! { pool_ids[0], pool_ids[1], pool_ids[2] },
        // };
        // pool_store.pools = hashmap! {
        //     pool_ids[0] => WeightedPool {
        //         tokens: vec![tokens[0], tokens[1], tokens[2]],
        //         ..other fields
        //     },
        //     pool_ids[1] => WeightedPool {
        //         tokens: vec![tokens[1], tokens[2]],
        //         ..other fields
        //     }
        //     pool_ids[2] => WeightedPool {
        //         tokens: vec![tokens[2]],
        //         ..other fields
        //     }
        // };

        // Testing ids_for_pools_containing_token_pair
        assert_eq!(
            registry
                .pool_ids_for_token_pair(&token_pairs[0])
                .collect::<HashSet<_>>(),
            hashset! { pool_ids[0] }
        );
        assert_eq!(
            registry
                .pool_ids_for_token_pair(&token_pairs[1])
                .collect::<HashSet<_>>(),
            hashset! { pool_ids[0], pool_ids[1] }
        );
        assert_eq!(
            registry
                .pool_ids_for_token_pair(&token_pairs[2])
                .collect::<HashSet<_>>(),
            hashset! { pool_ids[0] }
        );

        assert_eq!(
            registry.pool_ids_for_token_pairs(&hashset! { token_pairs[1], token_pairs[2] }),
            hashset! { pool_ids[0], pool_ids[1] }
        );

        // Testing pools_for
        assert!(registry.pools_by_id(&hashset! {}).is_empty());
        assert_eq!(
            registry.pools_by_id(&hashset! { pool_ids[0] }),
            vec![weighted_pools[0].clone()]
        );
        assert_eq!(
            registry.pools_by_id(&hashset! { pool_ids[1] }),
            vec![weighted_pools[1].clone()]
        );
        assert_eq!(
            registry.pools_by_id(&hashset! { pool_ids[2] }),
            vec![weighted_pools[2].clone()]
        );
        let res_0_1 = registry.pools_by_id(&hashset! { pool_ids[0], pool_ids[1] });
        assert_eq!(res_0_1.len(), 2);
        assert!(res_0_1.contains(&weighted_pools[0].clone()));
        assert!(res_0_1.contains(&weighted_pools[1].clone()));

        let res_0_2 = registry.pools_by_id(&hashset! { pool_ids[0], pool_ids[2] });
        assert_eq!(res_0_2.len(), 2);
        assert!(res_0_2.contains(&weighted_pools[0].clone()));
        assert!(res_0_2.contains(&weighted_pools[2].clone()));

        let res_1_2 = registry.pools_by_id(&hashset! { pool_ids[1], pool_ids[2] });
        assert_eq!(res_1_2.len(), 2);
        assert!(res_1_2.contains(&weighted_pools[1].clone()));
        assert!(res_1_2.contains(&weighted_pools[2].clone()));

        let res_0_1_2 = registry.pools_by_id(&hashset! { pool_ids[0], pool_ids[1], pool_ids[2]  });
        assert_eq!(res_0_1_2.len(), 3);
        assert!(res_0_1_2.contains(&weighted_pools[0].clone()));
        assert!(res_0_1_2.contains(&weighted_pools[1].clone()));
        assert!(res_0_1_2.contains(&weighted_pools[2].clone()));
    }
}
