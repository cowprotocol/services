//! Balancer pool registry initialization.
//!
//! This module contains a component used to initialize Balancer pool registries
//! with existing data in order to reduce the "cold start" time of the service.

use {
    super::graph_api::{BalancerSubgraphClient, RegisteredPools},
    anyhow::Result,
};

#[async_trait::async_trait]
pub trait PoolInitializing: Send + Sync {
    async fn initialize_pools(&self) -> Result<RegisteredPools>;
}

#[async_trait::async_trait]
impl PoolInitializing for BalancerSubgraphClient {
    async fn initialize_pools(&self) -> Result<RegisteredPools> {
        let registered_pools = self.get_registered_pools().await?;
        tracing::debug!(
            block = %registered_pools.fetched_block_number, pools = %registered_pools.pools.len(),
            "initialized registered pools",
        );

        Ok(registered_pools)
    }
}
