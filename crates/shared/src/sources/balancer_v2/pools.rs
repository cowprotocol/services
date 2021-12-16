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
use crate::Web3CallBatch;
use anyhow::Result;
use ethcontract::{BlockId, H256};
use futures::future::BoxFuture;

/// A Balancer pool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    /// The ID of the pool.
    pub id: H256,
    /// The pool-specific kind and state.
    pub kind: PoolKind,
}

/// Balancer pool state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolKind {
    Weighted(weighted::PoolState),
    Stable(stable::PoolState),
}

/// A Balancer factory indexing implementation.
#[mockall::automock(
    type PoolInfo = weighted::PoolInfo;
    type PoolState = weighted::PoolState;
)]
#[async_trait::async_trait]
pub trait FactoryIndexing: Send + Sync + 'static {
    /// The permanent pool info for this factory.
    ///
    /// This contains all pool information that never changes and only needs to
    /// be retrieved once. This data will be passed in when fetching the current
    /// pool state via `fetch_pool`.
    type PoolInfo: PoolIndexing;

    /// The current pool state for this factory.
    type PoolState;

    /// Augments the specified common pool info for this factory.
    ///
    /// This allows pool factories like the `WeightedPoolFactory` to add
    /// `weights` to the common pool info, since these are declared as
    /// `immuatble` in the smart contract and thus can never change and don't
    /// need to be re-fetched.
    ///
    /// The implementation is not expected to verify on-chain that the type of
    /// pool matches what it is expecting.
    ///
    /// Returns an error if fetching the augmented pool data fails.
    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo>;

    /// Fetches specialized pool state for the specified pool specialized info
    /// and common state.
    ///
    /// Additionally, a block spec and a batch call context is passed in to
    /// specify exactly the block number the state should be read for, and allow
    /// for more optimal performance when fetching a large number of pools.
    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        // This **needs** to be `'static` because of a `mockall` limitation
        // where we can't use other lifetimes here.
        // <https://github.com/asomers/mockall/issues/299>
        common_pool_state: BoxFuture<'static, common::PoolState>,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<Self::PoolState>>;
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
