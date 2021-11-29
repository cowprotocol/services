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

use super::{
    info_fetching::PoolInfoFetching,
    pool_factory::{common, FactoryIndexing},
    swap::fixed_point::Bfp,
};
use crate::event_handling::EventIndex;
use anyhow::Result;
use ethcontract::{H160, H256};
use model::TokenPair;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// PoolStorage represents in-memory storage of all deployed Balancer Pools
pub struct PoolStorage<F>
where
    F: FactoryIndexing,
{
    factory: F,

    /// Used for O(1) access to all pool_ids for a given token
    pools_by_token: HashMap<H160, HashSet<H256>>,
    /// All Registered Weighted Pools by ID
    pool_infos: HashMap<H256, F::PoolInfo>,
    /// The block the initial pools were fetched on. This block is considered
    /// reorg-safe and events prior to this block do not get replaced.
    initial_fetched_block: u64,
}

impl<F> PoolStorage<F>
where
    F: FactoryIndexing,
{
    pub fn new(factory: F, initial_pools: Vec<F::PoolInfo>) -> Self {
        initial_pools.into_iter().fold(
            Self {
                factory,
                pools_by_token: Default::default(),
                pool_infos: Default::default(),
                initial_fetched_block: 0,
            },
            |mut indexer, pool| {
                for token in pool.token_addresses() {
                    indexer
                        .pools_by_token
                        .entry(token)
                        .or_default()
                        .insert(pool.id());
                }
                indexer.initial_fetched_block =
                    cmp::max(indexer.initial_fetched_block, pool.block_created());
                indexer.pool_infos.insert(pool.id(), pool);

                indexer
            },
        )
    }

    #[cfg(test)]
    fn empty(data_fetcher: Arc<dyn PoolInfoFetching>) -> Self {
        Self::new(vec![], vec![], data_fetcher)
    }

    /// Returns all pools containing both tokens from `TokenPair`
    fn pool_ids_for_token_pair(&self, token_pair: TokenPair) -> impl Iterator<Item = H256> {
        let empty_set = HashSet::new();
        let (token0, token1) = token_pair.get();

        let pools0 = self.pools_by_token.get(&token0).unwrap_or(&empty_set);
        let pools1 = self.pools_by_token.get(&token1).unwrap_or(&empty_set);
        pools0.intersection(pools1).copied()
    }

    /// Given a collection of `TokenPair`, returns all pools containing at least
    /// one of the pairs.
    pub fn pool_ids_for_token_pairs(&self, token_pairs: &HashSet<TokenPair>) -> HashSet<H256> {
        token_pairs
            .into_iter()
            .flat_map(|pair| self.pool_ids_for_token_pair(pair))
            .collect()
    }

    /// Returns all pool infos by their IDs.
    pub fn pools_by_id(&self, pool_ids: &HashSet<H256>) -> Vec<F::PoolInfo> {
        pool_ids
            .iter()
            .filter_map(|pool_id| self.pool_infos.get(pool_id))
            .cloned()
            .collect()
    }

    /// Inserts new pools from their created events.
    pub async fn insert_pools(&mut self, pools: Vec<common::PoolInfo>) -> Result<()> {
        for pool in pools {
            let pool = self.factory.pool_info(pool).await?;
            for token in pool.token_addresses() {
                self.pools_by_token
                    .entry(token)
                    .or_default()
                    .insert(pool.id());
            }
            self.stable_pools.insert(pool.id(), pool);
        }

        Ok(())
    }

    /// Removes all pool creations from the specified block.
    pub fn remove_pools_newer_than_block(&mut self, delete_from_block_number: u64) {
        /* TODO(nlordell)
        tracing::debug!(
            "removing {} events from block number {}",
            events.len(),
            delete_from_block_number,
        );
        */

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

        let num_pools = self.pool_infos.len();
        self.pool_infos
            .retain(|_, pool| pool.block_created() < block);

        if num_pools == self.pool_infos.len() {
            // We didnt' actually remove any pools, so no need to rebuild the
            // tokens to pools map.
            return;
        }

        // Note that this could result in an empty set for some tokens.
        for pool_set in self.pools_by_token.values_mut() {
            pool_set.retain(|pool_id| self.pool_infos.contains_key(pool_id));
        }
    }

    pub fn last_event_block(&self) -> u64 {
        // Technically we could keep this updated more effectively in a field on balancer pools,
        // but the maintenance seems like more overhead that needs to be tested.
        self.pool_infos
            .values()
            .map(|pool| pool.block_created())
            .max()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::info_fetching::{
        CommonPoolInfo, MockPoolInfoFetching, StablePoolInfo, WeightedPoolInfo,
    };
    use maplit::{hashmap, hashset};
    use mockall::predicate::eq;

    pub type PoolInitData = (Vec<H256>, Vec<H160>, Vec<H160>, Vec<Bfp>, Vec<PoolCreated>);
    fn pool_init_data(start: usize, end: usize, pool_type: PoolType) -> PoolInitData {
        let pool_ids: Vec<H256> = (start..end + 1)
            .map(|i| H256::from_low_u64_be(i as u64))
            .collect();
        let pool_addresses: Vec<H160> = (start..end + 1)
            .map(|i| H160::from_low_u64_be(i as u64))
            .collect();
        let tokens: Vec<H160> = (start..end + 2)
            .map(|i| H160::from_low_u64_be(i as u64))
            .collect();
        let weights: Vec<Bfp> = (start..end + 2).map(|i| Bfp::from_wei(i.into())).collect();
        let creation_events: Vec<PoolCreated> = (start..end + 1)
            .map(|i| PoolCreated {
                pool_type,
                pool_address: pool_addresses[i],
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
                        pool_id: H256([1; 32]),
                        pool_address: H160([1; 20]),
                        tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                        scaling_exponents: vec![0, 0],
                        block_created: 0,
                    },
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                },
                RegisteredWeightedPool {
                    common: CommonPoolData {
                        pool_id: H256([2; 32]),
                        pool_address: H160([2; 20]),
                        tokens: vec![H160([0x11; 20]), H160([0x33; 20]), H160([0x77; 20])],
                        scaling_exponents: vec![0, 0],
                        block_created: 0,
                    },
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                },
            ],
            vec![RegisteredStablePool {
                common: CommonPoolData {
                    pool_id: H256([3; 32]),
                    pool_address: H160([3; 20]),
                    tokens: vec![H160([0x11; 20]), H160([0x77; 20])],
                    scaling_exponents: vec![0, 0],
                    block_created: 0,
                },
            }],
            Arc::new(MockPoolInfoFetching::new()),
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
    async fn insert_weighted_pool_events() {
        let n = 3usize;
        let (pool_ids, pool_addresses, tokens, weights, creation_events) =
            pool_init_data(0, n, PoolType::Weighted);

        let events: Vec<(EventIndex, PoolCreated)> = vec![
            (EventIndex::new(1, 0), creation_events[0]),
            (EventIndex::new(2, 0), creation_events[1]),
            (EventIndex::new(3, 0), creation_events[2]),
        ];

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        for i in 0..n {
            let expected_pool_data = WeightedPoolInfo {
                common: CommonPoolInfo {
                    pool_id: pool_ids[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    scaling_exponents: vec![0, 0],
                },
                weights: vec![weights[i], weights[i + 1]],
            };
            dummy_data_fetcher
                .expect_get_weighted_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }

        let mut pool_store = PoolStorage::empty(Arc::new(dummy_data_fetcher));
        pool_store.insert_events(events).await.unwrap();
        // Note that it is never expected that blocks for events will differ,
        // but in this test block_created for the pool is the first block it receives.
        assert_eq!(pool_store.last_event_block(), 3);
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
                pool_store.weighted_pools.get(&pool_ids[i]).unwrap(),
                &RegisteredWeightedPool {
                    common: CommonPoolData {
                        pool_id: pool_ids[i],
                        pool_address: pool_addresses[i],
                        tokens: vec![tokens[i], tokens[i + 1]],
                        scaling_exponents: vec![0, 0],
                        block_created: i as u64 + 1,
                    },
                    normalized_weights: vec![weights[i], weights[i + 1]],
                },
            );
        }
    }

    #[tokio::test]
    async fn insert_stable_pool_events() {
        let n = 3usize;
        let (pool_ids, pool_addresses, tokens, _, creation_events) =
            pool_init_data(0, n, PoolType::Stable);

        let events: Vec<(EventIndex, PoolCreated)> = vec![
            (EventIndex::new(1, 0), creation_events[0]),
            (EventIndex::new(2, 0), creation_events[1]),
            (EventIndex::new(3, 0), creation_events[2]),
        ];

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        for i in 0..n {
            let expected_pool_data = StablePoolInfo {
                common: CommonPoolInfo {
                    pool_id: pool_ids[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    scaling_exponents: vec![0, 0],
                },
            };
            dummy_data_fetcher
                .expect_get_stable_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }

        let mut pool_store = PoolStorage::empty(Arc::new(dummy_data_fetcher));
        pool_store.insert_events(events).await.unwrap();
        // Note that it is never expected that blocks for events will differ,
        // but in this test block_created for the pool is the first block it receives.
        assert_eq!(pool_store.last_event_block(), 3);
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
                pool_store.stable_pools.get(&pool_ids[i]).unwrap(),
                &RegisteredStablePool {
                    common: CommonPoolData {
                        pool_id: pool_ids[i],
                        pool_address: pool_addresses[i],
                        tokens: vec![tokens[i], tokens[i + 1]],
                        scaling_exponents: vec![0, 0],
                        block_created: i as u64 + 1
                    }
                },
            );
        }
    }

    #[tokio::test]
    async fn replace_weighted_pool_events() {
        let start_block = 0;
        let end_block = 5;
        let (pool_ids, pool_addresses, tokens, weights, creation_events) =
            pool_init_data(start_block, end_block, PoolType::Weighted);
        // Setup all the variables to initialize Balancer Pool State

        let converted_events: Vec<(EventIndex, PoolCreated)> = (start_block..end_block + 1)
            .map(|i| (EventIndex::new(i as u64, 0), creation_events[i]))
            .collect();

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        for i in start_block..end_block + 1 {
            let expected_pool_data = WeightedPoolInfo {
                common: CommonPoolInfo {
                    pool_id: pool_ids[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    scaling_exponents: vec![0, 0],
                },
                weights: vec![weights[i], weights[i + 1]],
            };
            dummy_data_fetcher
                .expect_get_weighted_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }

        // Have to prepare return data for new stuff before we pass on the data fetcher
        let new_pool_id = H256::from_low_u64_be(43110);
        let new_pool_address = H160::from_low_u64_be(42);
        let new_token = H160::from_low_u64_be(808);
        let new_weight = Bfp::from_wei(1337.into());
        let new_creation = PoolCreated {
            pool_type: PoolType::Weighted,
            pool_address: new_pool_address,
        };
        let new_event = (EventIndex::new(3, 0), new_creation);
        dummy_data_fetcher
            .expect_get_weighted_pool_data()
            .with(eq(new_pool_address))
            .returning(move |_| {
                Ok(WeightedPoolInfo {
                    common: CommonPoolInfo {
                        pool_id: new_pool_id,
                        tokens: vec![new_token],
                        scaling_exponents: vec![0],
                    },
                    weights: vec![new_weight],
                })
            });

        let mut pool_store = PoolStorage::empty(Arc::new(dummy_data_fetcher));
        pool_store.insert_events(converted_events).await.unwrap();
        // Let the tests begin!
        assert_eq!(pool_store.last_event_block(), end_block as u64);
        pool_store
            .replace_events_inner(3, vec![new_event])
            .await
            .unwrap();
        // Everything until block 3 is unchanged.
        for i in 0..3 {
            assert_eq!(
                pool_store.weighted_pools.get(&pool_ids[i]).unwrap(),
                &RegisteredWeightedPool {
                    common: CommonPoolData {
                        pool_id: pool_ids[i],
                        pool_address: pool_addresses[i],
                        tokens: vec![tokens[i], tokens[i + 1]],
                        scaling_exponents: vec![0, 0],
                        block_created: i as u64,
                    },
                    normalized_weights: vec![weights[i], weights[i + 1]],
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
            assert!(pool_store.weighted_pools.get(pool_id).is_none());
        }
        for token in tokens.iter().take(7).skip(4) {
            assert!(pool_store.pools_by_token.get(token).unwrap().is_empty());
        }

        let new_event_block = new_event.0.block_number;

        // All new data is included.
        assert_eq!(
            pool_store.weighted_pools.get(&new_pool_id).unwrap(),
            &RegisteredWeightedPool {
                common: CommonPoolData {
                    pool_id: new_pool_id,
                    pool_address: new_pool_address,
                    tokens: vec![new_token],
                    scaling_exponents: vec![0],
                    block_created: new_event_block,
                },
                normalized_weights: vec![new_weight],
            }
        );

        assert!(pool_store.pools_by_token.get(&new_token).is_some());
        assert_eq!(pool_store.last_event_block(), new_event_block);
    }

    #[tokio::test]
    async fn replace_stable_pool_events() {
        let start_block = 0;
        let end_block = 5;
        let (pool_ids, pool_addresses, tokens, _, creation_events) =
            pool_init_data(start_block, end_block, PoolType::Stable);
        // Setup all the variables to initialize Balancer Pool State
        let converted_events: Vec<(EventIndex, PoolCreated)> = (start_block..end_block + 1)
            .map(|i| (EventIndex::new(i as u64, 0), creation_events[i]))
            .collect();

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        for i in start_block..end_block + 1 {
            let expected_pool_data = StablePoolInfo {
                common: CommonPoolInfo {
                    pool_id: pool_ids[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    scaling_exponents: vec![0, 0],
                },
            };
            dummy_data_fetcher
                .expect_get_stable_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }

        // Have to prepare return data for new stuff before we pass on the data fetcher
        let new_pool_id = H256::from_low_u64_be(43110);
        let new_pool_address = H160::from_low_u64_be(42);
        let new_token = H160::from_low_u64_be(808);
        let new_creation = PoolCreated {
            pool_type: PoolType::Stable,
            pool_address: new_pool_address,
        };
        let new_event = (EventIndex::new(3, 0), new_creation);
        dummy_data_fetcher
            .expect_get_stable_pool_data()
            .with(eq(new_pool_address))
            .returning(move |_| {
                Ok(StablePoolInfo {
                    common: CommonPoolInfo {
                        pool_id: new_pool_id,
                        tokens: vec![new_token],
                        scaling_exponents: vec![0],
                    },
                })
            });

        let mut pool_store = PoolStorage::empty(Arc::new(dummy_data_fetcher));
        pool_store.insert_events(converted_events).await.unwrap();
        // Let the tests begin!
        assert_eq!(pool_store.last_event_block(), end_block as u64);
        pool_store
            .replace_events_inner(3, vec![new_event])
            .await
            .unwrap();
        // Everything until block 3 is unchanged.
        for i in 0..3 {
            assert_eq!(
                pool_store.stable_pools.get(&pool_ids[i]).unwrap(),
                &RegisteredStablePool {
                    common: CommonPoolData {
                        pool_id: pool_ids[i],
                        pool_address: pool_addresses[i],
                        tokens: vec![tokens[i], tokens[i + 1]],
                        scaling_exponents: vec![0, 0],
                        block_created: i as u64
                    }
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
            assert!(pool_store.stable_pools.get(pool_id).is_none());
        }
        for token in tokens.iter().take(7).skip(4) {
            assert!(pool_store.pools_by_token.get(token).unwrap().is_empty());
        }

        let new_event_block = new_event.0.block_number;

        // All new data is included.
        assert_eq!(
            pool_store.stable_pools.get(&new_pool_id).unwrap(),
            &RegisteredStablePool {
                common: CommonPoolData {
                    pool_id: new_pool_id,
                    pool_address: new_pool_address,
                    tokens: vec![new_token],
                    scaling_exponents: vec![0],
                    block_created: new_event_block,
                }
            }
        );

        assert!(pool_store.pools_by_token.get(&new_token).is_some());
        assert_eq!(pool_store.last_event_block(), new_event_block);
    }

    #[test]
    fn ids_for_pools_containing_token_pairs_() {
        let n = 3;
        let (pool_ids, pool_addresses, tokens, _, _) = pool_init_data(0, n, PoolType::Weighted);
        let token_pairs: Vec<TokenPair> = (0..n)
            .map(|i| TokenPair::new(tokens[i], tokens[(i + 1) % n]).unwrap())
            .collect();

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        // Have to load all expected data into fetcher before it is passed on.
        for i in 0..n {
            let expected_pool_data = WeightedPoolInfo {
                common: CommonPoolInfo {
                    pool_id: pool_ids[i],
                    tokens: tokens[i..n].to_owned(),
                    scaling_exponents: vec![],
                },
                weights: vec![],
            };
            dummy_data_fetcher
                .expect_get_weighted_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }
        let mut registry = PoolStorage::empty(Arc::new(dummy_data_fetcher));
        // Test the empty registry.
        for token_pair in token_pairs.iter().take(n) {
            assert!(registry
                .ids_for_pools_containing_token_pair(*token_pair)
                .is_empty());
        }

        // Now test non-empty pool with standard form.
        let mut weighted_pools = vec![];
        for i in 0..n {
            for pool_id in pool_ids.iter().take(i + 1) {
                // This is tokens[i] => { pool_id[0], pool_id[1], ..., pool_id[i] }
                let entry = registry.pools_by_token.entry(tokens[i]).or_default();
                entry.insert(*pool_id);
            }
            // This is weighted_pools[i] has tokens [tokens[i], tokens[i+1], ... , tokens[n]]
            weighted_pools.push(RegisteredWeightedPool {
                common: CommonPoolData {
                    pool_id: pool_ids[i],
                    tokens: tokens[i..n].to_owned(),
                    scaling_exponents: vec![],
                    block_created: 0,
                    pool_address: pool_addresses[i],
                },
                normalized_weights: vec![],
            });
            registry
                .weighted_pools
                .insert(pool_ids[i], weighted_pools[i].clone());
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
            registry.ids_for_pools_containing_token_pair(token_pairs[0]),
            hashset! {pool_ids[0]}
        );
        assert_eq!(
            registry.ids_for_pools_containing_token_pair(token_pairs[1]),
            hashset! { pool_ids[0], pool_ids[1] }
        );
        assert_eq!(
            registry.ids_for_pools_containing_token_pair(token_pairs[2]),
            hashset! {pool_ids[0]}
        );

        assert_eq!(
            registry
                .ids_for_pools_containing_token_pairs(hashset! { token_pairs[1], token_pairs[2] }),
            hashset! {pool_ids[0], pool_ids[1]}
        );

        // Testing pools_for
        assert!(registry.stable_pools_for(&hashset! {}).is_empty());
        assert!(registry.weighted_pools_for(&hashset! {}).is_empty());
        assert_eq!(
            registry.weighted_pools_for(&hashset! {pool_ids[0]}),
            vec![weighted_pools[0].clone()]
        );
        assert_eq!(
            registry.weighted_pools_for(&hashset! {pool_ids[1]}),
            vec![weighted_pools[1].clone()]
        );
        assert_eq!(
            registry.weighted_pools_for(&hashset! {pool_ids[2]}),
            vec![weighted_pools[2].clone()]
        );
        let res_0_1 = registry.weighted_pools_for(&hashset! {pool_ids[0], pool_ids[1]});
        assert_eq!(res_0_1.len(), 2);
        assert!(res_0_1.contains(&weighted_pools[0].clone()));
        assert!(res_0_1.contains(&weighted_pools[1].clone()));

        let res_0_2 = registry.weighted_pools_for(&hashset! {pool_ids[0], pool_ids[2]});
        assert_eq!(res_0_2.len(), 2);
        assert!(res_0_2.contains(&weighted_pools[0].clone()));
        assert!(res_0_2.contains(&weighted_pools[2].clone()));

        let res_1_2 = registry.weighted_pools_for(&hashset! {pool_ids[1], pool_ids[2]});
        assert_eq!(res_1_2.len(), 2);
        assert!(res_1_2.contains(&weighted_pools[1].clone()));
        assert!(res_1_2.contains(&weighted_pools[2].clone()));

        let res_012 =
            registry.weighted_pools_for(&hashset! {pool_ids[0], pool_ids[1], pool_ids[2] });
        assert_eq!(res_012.len(), 3);
        assert!(res_012.contains(&weighted_pools[0].clone()));
        assert!(res_012.contains(&weighted_pools[1].clone()));
        assert!(res_012.contains(&weighted_pools[2].clone()));
    }
}
