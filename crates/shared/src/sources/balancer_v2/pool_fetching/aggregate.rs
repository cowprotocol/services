//! An `InternalPoolFetching` implementation that fetches from multiple
//! `InternalPoolFetching`s.

use {
    super::internal::InternalPoolFetching,
    crate::{recent_block_cache::Block, sources::balancer_v2::pools::Pool},
    alloy::primitives::B256,
    anyhow::Result,
    futures::future,
    model::TokenPair,
    std::collections::HashSet,
};

/// An aggregate `InternalPoolFetching` implementation.
pub struct Aggregate {
    fetchers: Vec<Box<dyn InternalPoolFetching>>,
}

impl Aggregate {
    /// Creates a new aggregate pool fetcher from the specified fetchers.
    pub fn new(fetchers: Vec<Box<dyn InternalPoolFetching>>) -> Self {
        Aggregate { fetchers }
    }
}

#[async_trait::async_trait]
impl InternalPoolFetching for Aggregate {
    async fn pool_ids_for_token_pairs(&self, token_pairs: HashSet<TokenPair>) -> HashSet<B256> {
        future::join_all(
            self.fetchers
                .iter()
                .map(|fetcher| fetcher.pool_ids_for_token_pairs(token_pairs.clone())),
        )
        .await
        .into_iter()
        .flatten()
        .collect()
    }

    async fn pools_by_id(&self, pool_ids: HashSet<B256>, block: Block) -> Result<Vec<Pool>> {
        Ok(future::try_join_all(
            self.fetchers
                .iter()
                .map(|fetcher| fetcher.pools_by_id(pool_ids.clone(), block)),
        )
        .await?
        .into_iter()
        .flatten()
        .collect())
    }
}
