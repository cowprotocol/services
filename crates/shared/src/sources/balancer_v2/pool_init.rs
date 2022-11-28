//! Balancer pool registry initialization.
//!
//! This module contains a component used to initialize Balancer pool registries
//! with existing data in order to reduce the "cold start" time of the service.

use super::graph_api::{BalancerSubgraphClient, RegisteredPools};
use anyhow::{anyhow, bail, Result};
use contracts::BalancerV2Vault;
use ethcontract::{
    common::{contract::Network, DeploymentInformation},
    Contract,
};

#[async_trait::async_trait]
pub trait PoolInitializing: Send + Sync {
    async fn initialize_pools(&self) -> Result<RegisteredPools>;
}

/// A Balancer pool registry initializer that always returns empty pools.
///
/// This can be used to index all pools from events instead of relying on the
/// Balancer subgraph for example.
pub struct EmptyPoolInitializer(u64);

impl EmptyPoolInitializer {
    /// Creates a new empty pool initializer for the specified chain ID.
    #[cfg(test)]
    pub fn for_chain(chain_id: u64) -> Self {
        Self(chain_id)
    }
}

#[async_trait::async_trait]
impl PoolInitializing for EmptyPoolInitializer {
    async fn initialize_pools(&self) -> Result<RegisteredPools> {
        let fetched_block_number =
            deployment_block(BalancerV2Vault::raw_contract(), self.0).await?;
        Ok(RegisteredPools {
            fetched_block_number,
            ..Default::default()
        })
    }
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

fn deployment(contract: &Contract, chain_id: u64) -> Result<&Network> {
    contract
        .networks
        .get(&chain_id.to_string())
        // Note that we are conflating network IDs with chain IDs. In general
        // they cannot be considered the same, but for the networks that we
        // support (xDAI, Görli and Mainnet) they are.
        .ok_or_else(|| anyhow!("missing {} deployment for {}", contract.name, chain_id))
}

async fn deployment_block(contract: &Contract, chain_id: u64) -> Result<u64> {
    let deployment_info = deployment(contract, chain_id)?
        .deployment_information
        .ok_or_else(|| anyhow!("missing deployment information for {}", contract.name))?;

    match deployment_info {
        DeploymentInformation::BlockNumber(block) => Ok(block),
        DeploymentInformation::TransactionHash(tx) => {
            bail!("missing deployment block number for {}", tx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initializes_empty_pools() {
        let initializer = EmptyPoolInitializer(5);
        assert_eq!(
            initializer.initialize_pools().await.unwrap(),
            RegisteredPools {
                fetched_block_number: 4648099,
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn empty_initializer_errors_on_missing_deployment() {
        let initializer = EmptyPoolInitializer(999);
        assert!(initializer.initialize_pools().await.is_err());
    }
}
