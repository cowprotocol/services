//! Module providing an internal interface to enable composing pool fetching
//! strategies.

use {
    crate::{recent_block_cache::Block, sources::balancer_v2::pools::Pool},
    anyhow::Result,
    ethcontract::H256,
    model::TokenPair,
    std::collections::HashSet,
};

/// An internal trait implementing the required methods for implementing pool
/// fetching.
///
/// This allows us to compose different inner pool fetching strategies together.
#[async_trait::async_trait]
pub trait InternalPoolFetching: Send + Sync + 'static {
    /// Retrives all pool IDs that trade the specified pairs.
    async fn pool_ids_for_token_pairs(&self, token_pairs: HashSet<TokenPair>) -> HashSet<H256>;

    /// Fetches current pool states for the specified IDs and block.
    async fn pools_by_id(&self, pool_ids: HashSet<H256>, block: Block) -> Result<Vec<Pool>>;
}
