//! Balancer pool registry initialization.
//!
//! This module contains a component used to initialize Balancer pool registries
//! with existing data in order to reduce the "cold start" time of the service.

use crate::Web3;

use super::graph_api::{BalancerSubgraphClient, RegisteredPools};
use anyhow::{anyhow, bail, Context, Result};
use contracts::BalancerV2Vault;
use ethcontract::{
    common::{contract::Network, DeploymentInformation},
    BlockId, BlockNumber, Contract,
};

#[async_trait::async_trait]
pub trait PoolInitializing: Send + Sync {
    async fn initialize_pools(&self) -> Result<RegisteredPools>;
}

/// A Balancer pool registry initializer that always returns empty pools.
///
/// This can be used to index all pools from events instead of relying on the
/// Balancer subgraph for example.
pub struct EmptyPoolInitializer {
    chain_id: u64,
    web3: Web3,
}

impl EmptyPoolInitializer {
    /// Creates a new empty pool initializer for the specified chain ID.
    #[cfg(test)]
    pub fn for_chain(chain_id: u64, web3: Web3) -> Self {
        Self { chain_id, web3 }
    }
}

#[async_trait::async_trait]
impl PoolInitializing for EmptyPoolInitializer {
    async fn initialize_pools(&self) -> Result<RegisteredPools> {
        let fetched_block_number =
            deployment_block(BalancerV2Vault::raw_contract(), self.chain_id).await?;
        let fetched_block_hash = self
            .web3
            .eth()
            .block(BlockId::Number(BlockNumber::Number(
                fetched_block_number.into(),
            )))
            .await?
            .context("missing block")?
            .hash
            .context("missing hash")?;
        Ok(RegisteredPools {
            fetched_block: (fetched_block_number, fetched_block_hash),
            ..Default::default()
        })
    }
}

#[async_trait::async_trait]
impl PoolInitializing for BalancerSubgraphClient {
    async fn initialize_pools(&self) -> Result<RegisteredPools> {
        let registered_pools = self.get_registered_pools().await?;
        tracing::debug!(
            "initialized registered pools: block = {:?}, pools = {}",
            registered_pools.fetched_block,
            registered_pools.pools.len()
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
        // support (xDAI, Rinkeby, Görli and Mainnet) they are.
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
    use crate::transport::create_env_test_transport;
    use ethcontract::H256;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore]
    async fn initializes_empty_pools() {
        let transport = create_env_test_transport(); //for rinkeby
        let web3 = Web3::new(transport);
        let initializer = EmptyPoolInitializer { chain_id: 4, web3 };
        assert_eq!(
            initializer.initialize_pools().await.unwrap(),
            RegisteredPools {
                fetched_block: (
                    8441702,
                    H256::from_str(
                        "0xb97e739fd41be0d109163047099f04b1b03657befea31ec4f2adcb714e532f1e"
                    )
                    .unwrap()
                ),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    #[ignore]
    async fn empty_initializer_errors_on_missing_deployment() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let initializer = EmptyPoolInitializer {
            chain_id: 999,
            web3,
        };
        assert!(initializer.initialize_pools().await.is_err());
    }
}
