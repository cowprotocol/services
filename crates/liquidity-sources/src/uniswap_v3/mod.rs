//! Uniswap V3 baseline liquidity source implementation.
pub mod event_fetching;
pub mod graph_api;
pub mod pool_fetching;
pub mod pool_indexer;

use {
    self::graph_api::{PoolsWithTicks, RegisteredPools},
    alloy::primitives::Address,
    anyhow::Result,
    async_trait::async_trait,
};

/// Abstracts over places we can pull Uniswap V3 pool state + ticks from.
/// Currently there are two backends: the Uniswap V3 subgraph (historical,
/// queryable by block) and our own pool-indexer service (at-head only, with a
/// `wait_until` barrier to bound staleness).
///
/// Snapshot contract — both methods return data at a block *>=* `target_block`:
/// - Subgraph: honors `target_block` exactly via its `block: { number: ... }`
///   filter.
/// - Pool-indexer: blocks until its envelope's `block_number` has caught up to
///   `target_block`, then serves at-head data.
///
/// Each response carries `fetched_block_number` — the *actual* snapshot block.
/// Callers must use that (not `target_block`) as the event-replay anchor, since
/// the indexer's actual block can be later than `target_block`.
#[async_trait]
pub trait V3PoolDataSource: Send + Sync + 'static {
    /// Fetch the full set of pools the source knows about as of a block at or
    /// after `target_block`. `PoolData::ticks` is always `None` here — callers
    /// needing ticks must use [`Self::get_pools_with_ticks_by_ids`] separately.
    /// The split lets a cheap "what pools exist?" lookup skip the expensive
    /// tick fetch.
    async fn get_registered_pools(&self, target_block: u64) -> Result<RegisteredPools>;

    /// Fetch pools + their active ticks for the given pool addresses as of a
    /// block at or after `target_block`. The returned `fetched_block_number` is
    /// the actual snapshot block (`>= target_block`); callers should use it as
    /// the event-replay anchor.
    async fn get_pools_with_ticks_by_ids(
        &self,
        ids: &[Address],
        target_block: u64,
    ) -> Result<PoolsWithTicks>;
}
