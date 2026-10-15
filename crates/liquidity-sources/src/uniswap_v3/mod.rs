//! Uniswap V3 baseline liquidity source implementation.
pub mod event_fetching;
pub mod models;
pub mod pool_fetching;
pub mod pool_indexer;

use {
    self::models::{PoolsWithTicks, RegisteredPools},
    alloy::primitives::Address,
    anyhow::Result,
    async_trait::async_trait,
};

/// Abstracts over places we can pull Uniswap V3 pool state + ticks from. The
/// pool-indexer serves at-head data, with a `wait_until` barrier to bound
/// staleness.
///
/// Each response carries `fetched_block_number`: the actual snapshot block,
/// which is `>=` a requested [`BlockTarget::Number`]. Callers must use that
/// (not `target_block`) as the event-replay anchor, since the served block can
/// be later than the one requested.
#[async_trait]
pub trait V3PoolDataSource: Send + Sync + 'static {
    /// Fetch the full set of pools the source knows about as of `target_block`.
    /// `PoolData::ticks` is always `None` here — callers needing ticks must use
    /// [`Self::get_pools_with_ticks_by_ids`] separately. The split lets a cheap
    /// "what pools exist?" lookup skip the expensive tick fetch.
    async fn get_registered_pools(&self, target_block: BlockTarget) -> Result<RegisteredPools>;

    /// Fetch pools + their active ticks for the given pool addresses as of
    /// `target_block`. The returned `fetched_block_number` is the actual
    /// snapshot block (`>=` a requested [`BlockTarget::Number`]); callers
    /// should use it as the event-replay anchor.
    async fn get_pools_with_ticks_by_ids(
        &self,
        ids: &[Address],
        target_block: BlockTarget,
    ) -> Result<PoolsWithTicks>;
}

/// Which block a [`V3PoolDataSource`] anchors its snapshot to.
#[derive(Clone, Copy, Debug)]
pub enum BlockTarget {
    /// The latest block the source can serve, at-head without waiting.
    Latest,
    /// A specific block; the source returns data at or after it.
    Number(u64),
}
