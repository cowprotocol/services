use crate::pool_fetching::MAX_BATCH_SIZE;
use crate::{
    current_block::BlockRetrieving,
    event_handling::{BlockNumber, EventHandler, EventIndex, EventStoring},
    impl_event_retrieving,
    maintenance::Maintaining,
    Web3,
};
use anyhow::{anyhow, Context, Result};
use contracts::{
    balancer_v2_weighted_pool_factory::{
        self, event_data::PoolCreated as ContractPoolCreated, Event as ContractEvent,
    },
    BalancerV2Vault, BalancerV2WeightedPool, BalancerV2WeightedPoolFactory, ERC20,
};
use derivative::Derivative;
use ethcontract::batch::CallBatch;
use ethcontract::common::DeploymentInformation;
use ethcontract::{
    dyns::DynWeb3, Bytes, Event as EthContractEvent, EventMetadata, H160, H256, U256,
};
use futures::future::join_all;
use itertools::Itertools;
use mockall::*;
use model::TokenPair;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    ops::RangeInclusive,
};
use tokio::sync::Mutex;

#[derive(Copy, Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct PoolCreated {
    pub pool_address: H160,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct RegisteredWeightedPool {
    pub pool_id: H256,
    pub pool_address: H160,
    pub tokens: Vec<H160>,
    pub normalized_weights: Vec<U256>,
    pub scaling_exponents: Vec<u8>,
    block_created: u64,
}

impl RegisteredWeightedPool {
    /// Errors expected here are propagated from `get_pool_data`.
    async fn from_event(
        block_created: u64,
        creation: PoolCreated,
        data_fetcher: &dyn PoolDataFetching,
    ) -> Result<RegisteredWeightedPool> {
        let pool_address = creation.pool_address;
        let pool_data = data_fetcher.get_pool_data(pool_address).await?;
        return Ok(RegisteredWeightedPool {
            pool_id: pool_data.pool_id,
            pool_address,
            tokens: pool_data.tokens,
            normalized_weights: pool_data.weights,
            scaling_exponents: pool_data.scaling_exponents,
            block_created,
        });
    }
}

#[derive(Clone)]
pub struct WeightedPoolData {
    pool_id: H256,
    tokens: Vec<H160>,
    weights: Vec<U256>,
    scaling_exponents: Vec<u8>,
}

#[automock]
#[async_trait::async_trait]
trait PoolDataFetching: Send + Sync {
    async fn get_pool_data(&self, pool_address: H160) -> Result<WeightedPoolData>;
}

#[async_trait::async_trait]
impl PoolDataFetching for Web3 {
    /// Could result in ethcontract::{NodeError, MethodError or ContractError}
    async fn get_pool_data(&self, pool_address: H160) -> Result<WeightedPoolData> {
        let mut batch = CallBatch::new(self.transport());
        let pool_contract = BalancerV2WeightedPool::at(self, pool_address);
        // Need vault and pool_id before we can fetch tokens.
        let vault = BalancerV2Vault::deployed(&self).await?;
        let pool_id = H256::from(pool_contract.methods().get_pool_id().call().await?.0);

        // token_data and weight calls can be batched
        let token_data = vault
            .methods()
            .get_pool_tokens(Bytes(pool_id.0))
            .batch_call(&mut batch);
        let normalized_weights = pool_contract
            .methods()
            .get_normalized_weights()
            .batch_call(&mut batch);
        batch.execute_all(MAX_BATCH_SIZE).await;

        let tokens = token_data.await?.0;
        // This is already a batch call for a list of token decimals.
        let mut batch = CallBatch::new(self.transport());
        let futures = tokens
            .iter()
            .map(|address| {
                let erc20 = ERC20::at(&self, *address);
                erc20.methods().decimals().batch_call(&mut batch)
            })
            .collect::<Vec<_>>();
        batch.execute_all(MAX_BATCH_SIZE).await;
        let token_decimals = join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let scaling_exponents = token_decimals
            .iter()
            .map(|decimals| {
                // Note that balancer does not support tokens with more than 18 decimals
                // https://github.com/balancer-labs/balancer-v2-monorepo/blob/ce70f7663e0ac94b25ed60cb86faaa8199fd9e13/pkg/pool-utils/contracts/BasePool.sol#L497-L508
                18 - decimals
            })
            .collect();
        Ok(WeightedPoolData {
            pool_id,
            tokens,
            weights: normalized_weights.await?,
            scaling_exponents,
        })
    }
}

/// The BalancerPool struct represents in-memory storage of all deployed Balancer Pools
#[derive(Derivative)]
#[derivative(Debug)]
pub struct PoolRegistry {
    /// Used for O(1) access to all pool_ids for a given token
    pools_by_token: HashMap<H160, HashSet<H256>>,
    /// WeightedPool data for a given PoolId
    pools: HashMap<H256, RegisteredWeightedPool>,
    #[derivative(Debug = "ignore")]
    data_fetcher: Box<dyn PoolDataFetching>,
}

impl PoolRegistry {
    // Since all the fields are private, we expose helper methods to fetch relevant information
    /// Returns all pools containing both tokens from TokenPair
    pub fn pools_containing_token_pair(
        &self,
        token_pair: TokenPair,
    ) -> HashSet<RegisteredWeightedPool> {
        let empty_set = HashSet::new();
        let pools_0 = self
            .pools_by_token
            .get(&token_pair.get().0)
            .unwrap_or(&empty_set);
        let pools_1 = self
            .pools_by_token
            .get(&token_pair.get().1)
            .unwrap_or(&empty_set);
        pools_0
            .intersection(pools_1)
            .into_iter()
            .map(|pool_id| {
                self.pools
                    .get(pool_id)
                    .expect("failed iterating over known pools")
                    .clone()
            })
            .collect::<HashSet<RegisteredWeightedPool>>()
    }

    /// Given a collection of TokenPair, returns all pools containing at least one of the pairs.
    pub fn pools_containing_token_pairs(
        &self,
        token_pairs: HashSet<TokenPair>,
    ) -> HashSet<RegisteredWeightedPool> {
        token_pairs
            .into_iter()
            .flat_map(|pair| self.pools_containing_token_pair(pair))
            .unique_by(|pool| pool.pool_id)
            .collect()
    }

    async fn insert_events(&mut self, events: Vec<(EventIndex, PoolCreated)>) -> Result<()> {
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

    async fn replace_events(
        &mut self,
        delete_from_block_number: u64,
        events: Vec<(EventIndex, PoolCreated)>,
    ) -> Result<()> {
        self.delete_pools(delete_from_block_number)?;
        self.insert_events(events).await?;
        Ok(())
    }

    fn delete_pools(&mut self, delete_from_block_number: u64) -> Result<()> {
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
        Ok(())
    }

    fn last_event_block(&self) -> u64 {
        // Technically we could keep this updated more effectively in a field on balancer pools,
        // but the maintenance seems like more overhead that needs to be tested.
        self.pools
            .iter()
            .map(|(_, pool)| pool.block_created)
            .max()
            .unwrap_or(0)
    }

    fn contract_to_balancer_events(
        &self,
        contract_events: Vec<EthContractEvent<ContractEvent>>,
    ) -> Result<Vec<(EventIndex, PoolCreated)>> {
        contract_events
            .into_iter()
            .map(|EthContractEvent { data, meta }| {
                let meta = match meta {
                    Some(meta) => meta,
                    None => return Err(anyhow!("event without metadata")),
                };
                match data {
                    ContractEvent::PoolCreated(event) => convert_pool_created(&event, &meta),
                }
            })
            .collect::<Result<Vec<_>>>()
    }
}

pub struct BalancerEventUpdater(
    Mutex<EventHandler<DynWeb3, BalancerV2WeightedPoolFactoryContract, PoolRegistry>>,
);

impl BalancerEventUpdater {
    pub async fn new(contract: BalancerV2WeightedPoolFactory, pools: PoolRegistry) -> Result<Self> {
        let deployment_block = match contract.deployment_information() {
            Some(DeploymentInformation::BlockNumber(block_number)) => Some(block_number),
            Some(DeploymentInformation::TransactionHash(hash)) => Some(
                contract
                    .raw_instance()
                    .web3()
                    .block_number_from_tx_hash(hash)
                    .await?,
            ),
            None => None,
        };
        Ok(Self(Mutex::new(EventHandler::new(
            contract.raw_instance().web3(),
            BalancerV2WeightedPoolFactoryContract(contract),
            pools,
            deployment_block,
        ))))
    }
}

#[async_trait::async_trait]
impl EventStoring<ContractEvent> for PoolRegistry {
    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<ContractEvent>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()> {
        let balancer_events = self
            .contract_to_balancer_events(events)
            .context("failed to convert events")?;
        tracing::debug!(
            "replacing {} events from block number {}",
            balancer_events.len(),
            range.start().to_u64()
        );
        PoolRegistry::replace_events(self, 0, balancer_events).await?;
        Ok(())
    }

    async fn append_events(&mut self, events: Vec<EthContractEvent<ContractEvent>>) -> Result<()> {
        let balancer_events = self
            .contract_to_balancer_events(events)
            .context("failed to convert events")?;
        self.insert_events(balancer_events).await
    }

    async fn last_event_block(&self) -> Result<u64> {
        Ok(self.last_event_block())
    }
}

impl_event_retrieving! {
    pub BalancerV2WeightedPoolFactoryContract for balancer_v2_weighted_pool_factory
}

#[async_trait::async_trait]
impl Maintaining for BalancerEventUpdater {
    async fn run_maintenance(&self) -> Result<()> {
        self.0.run_maintenance().await
    }
}

fn convert_pool_created(
    creation: &ContractPoolCreated,
    meta: &EventMetadata,
) -> Result<(EventIndex, PoolCreated)> {
    Ok((
        EventIndex::from(meta),
        PoolCreated {
            pool_address: creation.pool,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashset;
    use mockall::predicate::eq;

    #[tokio::test]
    async fn balancer_insert_events() {
        let n = 3usize;
        let pool_ids: Vec<H256> = (0..n).map(|i| H256::from_low_u64_be(i as u64)).collect();
        let pool_addresses: Vec<H160> = (0..n).map(|i| H160::from_low_u64_be(i as u64)).collect();
        let tokens: Vec<H160> = (0..n + 1)
            .map(|i| H160::from_low_u64_be(i as u64))
            .collect();
        let weights: Vec<U256> = (0..n + 1).map(|i| U256::from(i as u64)).collect();
        let creation_events: Vec<PoolCreated> = (0..n)
            .map(|i| PoolCreated {
                pool_address: pool_addresses[i],
            })
            .collect();

        let events: Vec<(EventIndex, PoolCreated)> = vec![
            (EventIndex::new(1, 0), creation_events[0]),
            (EventIndex::new(2, 0), creation_events[1]),
            (EventIndex::new(3, 0), creation_events[2]),
        ];

        let mut dummy_data_fetcher = MockPoolDataFetching::new();

        for i in 0..n {
            let expected_pool_data = WeightedPoolData {
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

        let mut pool_store = PoolRegistry {
            pools_by_token: Default::default(),
            pools: Default::default(),
            data_fetcher: Box::new(dummy_data_fetcher),
        };
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
    async fn balancer_replace_events() {
        let start_block = 0;
        let end_block = 5;
        // Setup all the variables to initialize Balancer Pool State
        let pool_ids: Vec<H256> = (start_block..end_block + 1)
            .map(|i| H256::from_low_u64_be(i as u64))
            .collect();
        let pool_addresses: Vec<H160> = (start_block..end_block + 1)
            .map(|i| H160::from_low_u64_be(i as u64))
            .collect();
        let tokens: Vec<H160> = (start_block..end_block + 2)
            .map(|i| H160::from_low_u64_be(i as u64))
            .collect();
        let weights: Vec<U256> = (start_block..end_block + 2)
            .map(|i| U256::from(i as u64))
            .collect();
        let creation_events: Vec<PoolCreated> = (start_block..end_block + 1)
            .map(|i| PoolCreated {
                pool_address: pool_addresses[i],
            })
            .collect();

        let converted_events: Vec<(EventIndex, PoolCreated)> = (start_block..end_block + 1)
            .map(|i| (EventIndex::new(i as u64, 0), creation_events[i]))
            .collect();

        let mut dummy_data_fetcher = MockPoolDataFetching::new();
        for i in start_block..end_block + 1 {
            let expected_pool_data = WeightedPoolData {
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
        let new_weight = U256::from(1337);
        let new_creation = PoolCreated {
            pool_address: new_pool_address,
        };
        let new_event = (EventIndex::new(3, 0), new_creation);
        dummy_data_fetcher
            .expect_get_pool_data()
            .with(eq(new_pool_address))
            .returning(move |_| {
                Ok(WeightedPoolData {
                    pool_id: new_pool_id,
                    tokens: vec![new_token],
                    weights: vec![new_weight],
                    scaling_exponents: vec![0],
                })
            });

        let mut pool_store = PoolRegistry {
            pools_by_token: Default::default(),
            pools: Default::default(),
            data_fetcher: Box::new(dummy_data_fetcher),
        };
        pool_store.insert_events(converted_events).await.unwrap();
        // Let the tests begin!
        assert_eq!(pool_store.last_event_block(), end_block as u64);
        pool_store.replace_events(3, vec![new_event]).await.unwrap();

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
    fn pools_containing_pair_test() {
        let n = 3;
        let pool_ids: Vec<H256> = (0..n).map(|i| H256::from_low_u64_be(i as u64)).collect();
        let pool_addresses: Vec<H160> = (0..n)
            .map(|i| H160::from_low_u64_be(2 * i as u64))
            .collect();
        let tokens: Vec<H160> = (0..n)
            .map(|i| H160::from_low_u64_be(2 * i as u64 + 1))
            .collect();
        let token_pairs: Vec<TokenPair> = (0..n)
            .map(|i| TokenPair::new(tokens[i], tokens[(i + 1) % n]).unwrap())
            .collect();

        let mut dummy_data_fetcher = MockPoolDataFetching::new();
        // Have to load all expected data into fetcher before it is passed on.
        for i in 0..n {
            let expected_pool_data = WeightedPoolData {
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
        // Test the empty pool.
        let mut pool_store = PoolRegistry {
            pools_by_token: Default::default(),
            pools: Default::default(),
            data_fetcher: Box::new(dummy_data_fetcher),
        };
        for token_pair in token_pairs.iter().take(n) {
            assert!(pool_store
                .pools_containing_token_pair(*token_pair)
                .is_empty());
        }

        // Now test non-empty pool with standard form.
        let mut weighted_pools = vec![];
        for i in 0..n {
            for pool_id in pool_ids.iter().take(i + 1) {
                // This is tokens[i] => { pool_id[0], pool_id[1], ..., pool_id[i] }
                let entry = pool_store.pools_by_token.entry(tokens[i]).or_default();
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
            pool_store
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

        assert_eq!(
            pool_store.pools_containing_token_pair(token_pairs[0]),
            hashset! { weighted_pools[0].clone() }
        );
        assert_eq!(
            pool_store.pools_containing_token_pair(token_pairs[1]),
            hashset! { weighted_pools[1].clone(), weighted_pools[0].clone() }
        );
        assert_eq!(
            pool_store.pools_containing_token_pair(token_pairs[2]),
            hashset! { weighted_pools[0].clone() }
        );
    }
}
