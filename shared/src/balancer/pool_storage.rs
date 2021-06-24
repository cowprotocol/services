//! Pool Storage contains all the essential models required by the balancer module for operating
//! between different knowledge-levels of pool information.
//!
//! To briefly list and describe each of the models.
//!
//! 1. `PoolCreated`:
//!     contains only the `pool_address` as this is the only information known about the pool
//!     at the time of event emission from the pool's factory contract.
//!
//! 2. `RegisteredWeightedPool`:
//!     contains all constant/static information about the pool (that which is not block-sensitive).
//!     That is, `pool_id`, `address`, `tokens`, `normalized_weights`, `scaling_exponents` and `block_created`.
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
//! 4. `WeightedPool`:
//!     This is the public facing pool structure returned by the `PoolFetcher` consisting of all
//!     the pool's most recent information (both static and dynamic).
//!     Essentially, this is all the relevant data from `RegisteredWeightedPool` along with the
//!     current balances of each of the pool's tokens (aka the pool's "reserves").
//!
//! Tests included here are those pertaining to the expected functionality of `PoolStorage`
use crate::{
    balancer::{info_fetching::PoolInfoFetching, swap::fixed_point::Bfp},
    event_handling::EventIndex,
};
use anyhow::Result;
use derivative::Derivative;
use ethcontract::{H160, H256, U256};
use model::TokenPair;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct PoolTokenState {
    pub balance: U256,
    pub weight: Bfp,
    pub scaling_exponent: u8,
}

#[derive(Clone)]
pub struct WeightedPool {
    pub pool_id: H256,
    pub pool_address: H160,
    pub swap_fee_percentage: U256,
    pub reserves: HashMap<H160, PoolTokenState>,
}

impl WeightedPool {
    pub fn new(
        pool_data: RegisteredWeightedPool,
        balances: Vec<U256>,
        swap_fee_percentage: U256,
    ) -> Self {
        let mut reserves = HashMap::new();
        // We expect the weight and token indices are aligned with balances returned from EVM query.
        // If necessary we would also pass the tokens along with the query result,
        // use them and fetch the weights from the registry by token address.
        for (i, balance) in balances.into_iter().enumerate() {
            reserves.insert(
                pool_data.tokens[i],
                PoolTokenState {
                    balance,
                    weight: pool_data.normalized_weights[i],
                    scaling_exponent: pool_data.scaling_exponents[i],
                },
            );
        }
        WeightedPool {
            pool_id: pool_data.pool_id,
            pool_address: pool_data.pool_address,
            swap_fee_percentage,
            reserves,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegisteredWeightedPool {
    pub pool_id: H256,
    pub pool_address: H160,
    pub tokens: Vec<H160>,
    pub normalized_weights: Vec<Bfp>,
    pub scaling_exponents: Vec<u8>,
    pub block_created: u64,
}

#[derive(Copy, Debug, Default, Clone, Eq, PartialEq)]
pub struct PoolCreated {
    pub pool_address: H160,
}

impl RegisteredWeightedPool {
    /// Errors expected here are propagated from `get_pool_data`.
    pub async fn from_event(
        block_created: u64,
        creation: PoolCreated,
        data_fetcher: &dyn PoolInfoFetching,
    ) -> Result<RegisteredWeightedPool> {
        let pool_address = creation.pool_address;
        let pool_data = data_fetcher.get_pool_data(pool_address).await?;
        Ok(RegisteredWeightedPool {
            pool_id: pool_data.pool_id,
            pool_address,
            tokens: pool_data.tokens,
            normalized_weights: pool_data.weights,
            scaling_exponents: pool_data.scaling_exponents,
            block_created,
        })
    }
}

/// PoolStorage represents in-memory storage of all deployed Balancer Pools
#[derive(Derivative)]
#[derivative(Debug)]
pub struct PoolStorage {
    /// Used for O(1) access to all pool_ids for a given token
    pools_by_token: HashMap<H160, HashSet<H256>>,
    /// WeightedPool data for a given PoolId
    pools: HashMap<H256, RegisteredWeightedPool>,
    #[derivative(Debug = "ignore")]
    data_fetcher: Box<dyn PoolInfoFetching>,
}

impl PoolStorage {
    pub fn new(
        initial_pools: Vec<RegisteredWeightedPool>,
        data_fetcher: Box<dyn PoolInfoFetching>,
    ) -> Self {
        let mut pools_by_token = HashMap::<_, HashSet<_>>::new();
        let mut pools = HashMap::new();
        for pool in initial_pools {
            for token in &pool.tokens {
                pools_by_token
                    .entry(*token)
                    .or_default()
                    .insert(pool.pool_id);
            }
            pools.insert(pool.pool_id, pool);
        }

        PoolStorage {
            pools_by_token,
            pools,
            data_fetcher,
        }
    }

    #[cfg(test)]
    fn empty(data_fetcher: Box<dyn PoolInfoFetching>) -> Self {
        Self::new(vec![], data_fetcher)
    }

    /// Returns all pools containing both tokens from `TokenPair`
    pub fn ids_for_pools_containing_token_pair(&self, token_pair: TokenPair) -> HashSet<H256> {
        let empty_set = HashSet::new();
        let pools_0 = self
            .pools_by_token
            .get(&token_pair.get().0)
            .unwrap_or(&empty_set);
        let pools_1 = self
            .pools_by_token
            .get(&token_pair.get().1)
            .unwrap_or(&empty_set);
        pools_0.intersection(pools_1).copied().collect()
    }

    /// Given a collection of `TokenPair`, returns all pools containing at least one of the pairs.
    pub fn ids_for_pools_containing_token_pairs(
        &self,
        token_pairs: HashSet<TokenPair>,
    ) -> HashSet<H256> {
        token_pairs
            .into_iter()
            .flat_map(|pair| self.ids_for_pools_containing_token_pair(pair))
            .collect()
    }

    pub fn pools_for(&self, pool_ids: &HashSet<H256>) -> Vec<RegisteredWeightedPool> {
        self.pools
            .iter()
            .filter_map(|(pool_id, pool)| {
                if pool_ids.contains(pool_id) {
                    Some(pool)
                } else {
                    None
                }
            })
            .cloned()
            .collect()
    }

    pub async fn insert_events(&mut self, events: Vec<(EventIndex, PoolCreated)>) -> Result<()> {
        for (index, creation) in events {
            let weighted_pool = RegisteredWeightedPool::from_event(
                index.block_number,
                creation,
                &*self.data_fetcher,
            )
            .await?;
            let pool_id = weighted_pool.pool_id;
            self.pools.insert(pool_id, weighted_pool.clone());
            for token in weighted_pool.tokens {
                self.pools_by_token
                    .entry(token)
                    .or_default()
                    .insert(pool_id);
            }
        }
        Ok(())
    }

    pub async fn replace_events_inner(
        &mut self,
        delete_from_block_number: u64,
        events: Vec<(EventIndex, PoolCreated)>,
    ) -> Result<()> {
        tracing::debug!(
            "replacing {} events from block number {}",
            events.len(),
            delete_from_block_number,
        );
        self.delete_pools(delete_from_block_number);
        self.insert_events(events).await?;
        Ok(())
    }

    fn delete_pools(&mut self, delete_from_block_number: u64) {
        self.pools
            .retain(|_, pool| pool.block_created < delete_from_block_number);
        // Note that this could result in an empty set for some tokens.
        let retained_pool_ids: HashSet<H256> = self.pools.keys().copied().collect();
        for (_, pool_set) in self.pools_by_token.iter_mut() {
            *pool_set = pool_set
                .intersection(&retained_pool_ids)
                .cloned()
                .collect::<HashSet<H256>>();
        }
    }

    pub fn last_event_block(&self) -> u64 {
        // Technically we could keep this updated more effectively in a field on balancer pools,
        // but the maintenance seems like more overhead that needs to be tested.
        self.pools
            .iter()
            .map(|(_, pool)| pool.block_created)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balancer::info_fetching::{MockPoolInfoFetching, WeightedPoolInfo};
    use maplit::{hashmap, hashset};
    use mockall::predicate::eq;

    pub type PoolInitData = (Vec<H256>, Vec<H160>, Vec<H160>, Vec<Bfp>, Vec<PoolCreated>);
    fn pool_init_data(start: usize, end: usize) -> PoolInitData {
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
                    pool_id: H256([1; 32]),
                    pool_address: H160([1; 20]),
                    tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                    scaling_exponents: vec![0, 0],
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                    block_created: 0,
                },
                RegisteredWeightedPool {
                    pool_id: H256([2; 32]),
                    pool_address: H160([2; 20]),
                    tokens: vec![H160([0x11; 20]), H160([0x33; 20]), H160([0x77; 20])],
                    scaling_exponents: vec![0, 0],
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                    block_created: 0,
                },
            ],
            Box::new(MockPoolInfoFetching::new()),
        );

        assert_eq!(
            storage.pools_by_token,
            hashmap! {
                H160([0x11; 20]) => hashset![H256([1; 32]), H256([2; 32])],
                H160([0x22; 20]) => hashset![H256([1; 32])],
                H160([0x33; 20]) => hashset![H256([2; 32])],
                H160([0x77; 20]) => hashset![H256([2; 32])],
            }
        );
    }

    #[tokio::test]
    async fn insert_events() {
        let n = 3usize;
        let (pool_ids, pool_addresses, tokens, weights, creation_events) = pool_init_data(0, n);

        let events: Vec<(EventIndex, PoolCreated)> = vec![
            (EventIndex::new(1, 0), creation_events[0]),
            (EventIndex::new(2, 0), creation_events[1]),
            (EventIndex::new(3, 0), creation_events[2]),
        ];

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        for i in 0..n {
            let expected_pool_data = WeightedPoolInfo {
                pool_id: pool_ids[i],
                tokens: vec![tokens[i], tokens[i + 1]],
                weights: vec![weights[i], weights[i + 1]],
                scaling_exponents: vec![0, 0],
            };
            dummy_data_fetcher
                .expect_get_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }

        let mut pool_store = PoolStorage::empty(Box::new(dummy_data_fetcher));
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
                pool_store.pools.get(&pool_ids[i]).unwrap(),
                &RegisteredWeightedPool {
                    pool_id: pool_ids[i],
                    pool_address: pool_addresses[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    normalized_weights: vec![weights[i], weights[i + 1]],
                    scaling_exponents: vec![0, 0],
                    block_created: i as u64 + 1
                },
                "failed assertion at index {}",
                i
            );
        }
    }

    #[tokio::test]
    async fn replace_events() {
        let start_block = 0;
        let end_block = 5;
        let (pool_ids, pool_addresses, tokens, weights, creation_events) =
            pool_init_data(start_block, end_block);
        // Setup all the variables to initialize Balancer Pool State

        let converted_events: Vec<(EventIndex, PoolCreated)> = (start_block..end_block + 1)
            .map(|i| (EventIndex::new(i as u64, 0), creation_events[i]))
            .collect();

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        for i in start_block..end_block + 1 {
            let expected_pool_data = WeightedPoolInfo {
                pool_id: pool_ids[i],
                tokens: vec![tokens[i], tokens[i + 1]],
                weights: vec![weights[i], weights[i + 1]],
                scaling_exponents: vec![0, 0],
            };
            dummy_data_fetcher
                .expect_get_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }

        // Have to prepare return data for new stuff before we pass on the data fetcher
        let new_pool_id = H256::from_low_u64_be(43110);
        let new_pool_address = H160::from_low_u64_be(42);
        let new_token = H160::from_low_u64_be(808);
        let new_weight = Bfp::from_wei(1337.into());
        let new_creation = PoolCreated {
            pool_address: new_pool_address,
        };
        let new_event = (EventIndex::new(3, 0), new_creation);
        dummy_data_fetcher
            .expect_get_pool_data()
            .with(eq(new_pool_address))
            .returning(move |_| {
                Ok(WeightedPoolInfo {
                    pool_id: new_pool_id,
                    tokens: vec![new_token],
                    weights: vec![new_weight],
                    scaling_exponents: vec![0],
                })
            });

        let mut pool_store = PoolStorage::empty(Box::new(dummy_data_fetcher));
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
                pool_store.pools.get(&pool_ids[i]).unwrap(),
                &RegisteredWeightedPool {
                    pool_id: pool_ids[i],
                    pool_address: pool_addresses[i],
                    tokens: vec![tokens[i], tokens[i + 1]],
                    normalized_weights: vec![weights[i], weights[i + 1]],
                    scaling_exponents: vec![0, 0],
                    block_created: i as u64
                },
                "assertion failed at index {}",
                i
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

        let new_event_block = new_event.0.block_number;

        // All new data is included.
        assert_eq!(
            pool_store.pools.get(&new_pool_id).unwrap(),
            &RegisteredWeightedPool {
                pool_id: new_pool_id,
                pool_address: new_pool_address,
                tokens: vec![new_token],
                normalized_weights: vec![new_weight],
                scaling_exponents: vec![0],
                block_created: new_event_block
            }
        );

        assert!(pool_store.pools_by_token.get(&new_token).is_some());
        assert_eq!(pool_store.last_event_block(), new_event_block);
    }

    #[test]
    fn ids_for_pools_containing_token_pairs_() {
        let n = 3;
        let (pool_ids, pool_addresses, tokens, _, _) = pool_init_data(0, n);
        let token_pairs: Vec<TokenPair> = (0..n)
            .map(|i| TokenPair::new(tokens[i], tokens[(i + 1) % n]).unwrap())
            .collect();

        let mut dummy_data_fetcher = MockPoolInfoFetching::new();
        // Have to load all expected data into fetcher before it is passed on.
        for i in 0..n {
            let expected_pool_data = WeightedPoolInfo {
                pool_id: pool_ids[i],
                tokens: tokens[i..n].to_owned(),
                weights: vec![],
                scaling_exponents: vec![],
            };
            dummy_data_fetcher
                .expect_get_pool_data()
                .with(eq(pool_addresses[i]))
                .returning(move |_| Ok(expected_pool_data.clone()));
        }
        let mut registry = PoolStorage::empty(Box::new(dummy_data_fetcher));
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
                pool_id: pool_ids[i],
                tokens: tokens[i..n].to_owned(),
                normalized_weights: vec![],
                scaling_exponents: vec![],
                block_created: 0,
                pool_address: pool_addresses[i],
            });
            registry
                .pools
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

        // Testing pools_for
        assert!(registry.pools_for(&hashset! {}).is_empty());
        assert_eq!(
            registry.pools_for(&hashset! {pool_ids[0]}),
            vec![weighted_pools[0].clone()]
        );
        assert_eq!(
            registry.pools_for(&hashset! {pool_ids[1]}),
            vec![weighted_pools[1].clone()]
        );
        assert_eq!(
            registry.pools_for(&hashset! {pool_ids[2]}),
            vec![weighted_pools[2].clone()]
        );
        let res_0_1 = registry.pools_for(&hashset! {pool_ids[0], pool_ids[1]});
        assert_eq!(res_0_1.len(), 2);
        assert!(res_0_1.contains(&weighted_pools[0]));
        assert!(res_0_1.contains(&weighted_pools[1]));

        let res_0_2 = registry.pools_for(&hashset! {pool_ids[0], pool_ids[2]});
        assert_eq!(res_0_2.len(), 2);
        assert!(res_0_2.contains(&weighted_pools[0]));
        assert!(res_0_2.contains(&weighted_pools[2]));

        let res_1_2 = registry.pools_for(&hashset! {pool_ids[1], pool_ids[2]});
        assert_eq!(res_1_2.len(), 2);
        assert!(res_1_2.contains(&weighted_pools[1]));
        assert!(res_1_2.contains(&weighted_pools[2]));

        let res_012 = registry.pools_for(&hashset! {pool_ids[0], pool_ids[1], pool_ids[2] });
        assert_eq!(res_012.len(), 3);
        assert!(res_012.contains(&weighted_pools[0]));
        assert!(res_012.contains(&weighted_pools[1]));
        assert!(res_012.contains(&weighted_pools[2]));
    }
}
