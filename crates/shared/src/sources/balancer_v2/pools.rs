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
use anyhow::Result;

/// A Balancer factory indexing implementation.
#[mockall::automock(type PoolInfo = weighted::PoolInfo;)]
#[async_trait::async_trait]
pub trait FactoryIndexing: Send + Sync + 'static {
    /// The permanent pool info for this.
    ///
    /// This contains all pool information that never changes and only needs to
    /// be retrieved once. This data will be passed in when fetching the current
    /// pool state via `fetch_pool`.
    type PoolInfo: PoolIndexing;

    /// Augments the specified common pool info for this factory.
    ///
    /// This allows pool factories like the `WeightedPoolFactory` to add
    /// `weights` to the common pool info, since these are declared as
    /// `immuatble` in the smart contract and thus can never change and don't
    /// need to be re-fetched.
    ///
    /// Returns an error if fetching the augmented pool data fails.
    async fn augment_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo>;
}

/// Required information needed for indexing pools.
pub trait PoolIndexing: Clone + Send + Sync + 'static {
    /// Creates a new instance from a pool
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self>
    where
        Self: Sized;

    /// Gets the common pool data.
    fn common(&self) -> &common::PoolInfo;
}
