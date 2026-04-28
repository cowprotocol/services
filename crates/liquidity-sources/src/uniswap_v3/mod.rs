//! Uniswap V3 baseline liquidity source implementation.
pub mod event_fetching;
pub mod graph_api;
pub mod pool_fetching;
pub mod pool_indexer;

use {
    self::graph_api::{PoolData, RegisteredPools},
    alloy::primitives::Address,
    anyhow::Result,
    async_trait::async_trait,
};

/// Abstracts over places we can pull Uniswap V3 pool state + ticks from.
/// Currently there are two backends: the Uniswap V3 subgraph (historical,
/// queryable by block) and our own pool-indexer service (at-head only).
#[async_trait]
pub trait V3PoolDataSource: Send + Sync + 'static {
    /// Fetch the full set of pools the source knows about, tagged with the
    /// block number the snapshot was taken at. `PoolData::ticks` is always
    /// `None` here — callers needing ticks must use
    /// [`Self::get_pools_with_ticks_by_ids`] separately. The split lets a
    /// cheap "what pools exist?" lookup skip the expensive tick fetch.
    async fn get_registered_pools(&self) -> Result<RegisteredPools>;

    /// Fetch pools + their active ticks for the given pool addresses. The
    /// `block_number` hint is honored by sources that support historical
    /// queries (subgraph); sources that only expose head data (pool-indexer)
    /// ignore it and return at-head data.
    async fn get_pools_with_ticks_by_ids(
        &self,
        ids: &[Address],
        block_number: u64,
    ) -> Result<Vec<PoolData>>;
}
