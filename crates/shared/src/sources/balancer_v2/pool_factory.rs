//! Abstraction around a Balancer pool factory.
//!
//! These factories are used for indexing pool events, fetching "permanent" pool
//! information (such as token addresses for pools) as well as pool "state"
//! (such as current reserve balances, and current swap fee).
//!
//! This abstraction is provided in a way to simplify adding new Balancer pool
//! types by just implementing the required `BalancerFactory` trait.

pub mod common;
pub mod stable;
pub mod weighted;
pub mod weighted_2token;

use super::graph_api::PoolData;
use crate::event_handling::EventRetrieving;
use anyhow::Result;
use ethcontract::H160;

/// A Balancer factory indexing implementation.
#[async_trait::async_trait]
pub trait FactoryIndexing {
    /// The permanent pool info for this.
    ///
    /// This contains all pool information that never changes and only needs to
    /// be retrieved once. This data will be passed in when fetching the current
    /// pool state via `fetch_pool`.
    type PoolInfo: PoolIndexing;

    /// Returns the address of the pool factory.
    fn factory_address(&self) -> H160;

    /// Retrive the permanent pool info for the corresponding `PoolCreated`
    /// event.
    async fn pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo>;
}

/// Required information needed for indexing pools.
pub trait PoolIndexing {
    /// Creates a new instance from a pool
    fn from_graph_data(pool: PoolData, block_created: u64) -> Result<Self>
    where
        Self: Sized;

    /// Gets the common pool data.
    fn common(&self) -> &common::PoolInfo;
}

/*

/// A factory indexer.
///
/// This can be shared across all Balancer pool factory types.
pub struct FactoryIndexer<F>
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

impl<F> FactoryIndexer<F>
where
    F: FactoryIndexing,
{
    /// Creates a new factory indexer for the spcified factory and initial pools.
    pub fn new(
        factory: F,
        initial_pools: Vec<PoolData>,
        initial_fetched_block: u64,
    ) -> Result<Self> {
        initial_pools.into_iter().try_fold(
            Self {
                factory,
                pools_by_token: Default::default(),
                pool_infos: Default::default(),
                initial_fetched_block,
            },
            |mut indexer, pool| {
                let pool = F::PoolInfo::from_graph_data(pool, fetched_block_number)?;

                for token in pool.token_addresses() {
                    indexer
                        .pools_by_token
                        .entry(token)
                        .or_default()
                        .insert(pool.id());
                }
                indexer.pool_infos.insert(pool.id(), pool);

                Ok(indexer)
            },
        )
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

    /// Retrieves the last block that events were indexed for.
    pub fn last_indexed_block(&self) -> u64 {
        self.pool_infos
            .values()
            .map(|pool| pool.block_created())
            .max()
            .unwrap_or_default()
    }
}

*/
