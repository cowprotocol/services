//! Balancer pool registry initialization.
//!
//! This module contains a component used to initialize Balancer pool registries
//! with existing data in order to reduce the "cold start" time of the service.

use super::{
    graph_api::{BalancerSubgraphClient, PoolType, RegisteredPools},
    pool_storage::{RegisteredStablePool, RegisteredWeightedPool},
    pools::PoolIndexing,
};
use anyhow::{anyhow, bail, Result};
use contracts::{
    BalancerV2StablePoolFactory, BalancerV2Vault, BalancerV2WeightedPool2TokensFactory,
    BalancerV2WeightedPoolFactory,
};
use ethcontract::{
    common::{contract::Network, DeploymentInformation},
    Contract, H160,
};
use reqwest::Client;

#[derive(Debug, Default, PartialEq)]
pub struct BalancerRegisteredPools {
    pub weighted_pools: Vec<RegisteredWeightedPool>,
    pub weighted_2token_pools: Vec<RegisteredWeightedPool>,
    pub stable_pools: Vec<RegisteredStablePool>,
    pub fetched_block_number: u64,
}

#[async_trait::async_trait]
pub trait PoolInitializing: Send + Sync {
    async fn initialize_pools(&self) -> Result<BalancerRegisteredPools>;
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
    async fn initialize_pools(&self) -> Result<BalancerRegisteredPools> {
        let fetched_block_number =
            deployment_block(BalancerV2Vault::raw_contract(), self.0).await?;
        Ok(BalancerRegisteredPools {
            fetched_block_number,
            ..Default::default()
        })
    }
}

/// A pool initializer that uses the Balancer subgraph.
pub struct SubgraphPoolInitializer {
    deployment: PoolFactoryDeployment,
    client: Box<dyn BalancerSubgraph>,
}

impl SubgraphPoolInitializer {
    pub fn new(chain_id: u64, client: Client) -> Result<Self> {
        Ok(Self {
            deployment: PoolFactoryDeployment::for_chain(chain_id)?,
            client: Box::new(BalancerSubgraphClient::for_chain(chain_id, client)?),
        })
    }
}

#[async_trait::async_trait]
impl PoolInitializing for SubgraphPoolInitializer {
    async fn initialize_pools(&self) -> Result<BalancerRegisteredPools> {
        let graph_pools = self.client.registered_pools().await?;
        let registered_pools =
            BalancerRegisteredPools::from_graph_pool_data(&self.deployment, graph_pools)?;

        tracing::debug!(
            "initialized registered pools (block {}: {} Weighted, {} Weighted2Token, {} Stable)",
            registered_pools.fetched_block_number,
            registered_pools.weighted_pools.len(),
            registered_pools.weighted_2token_pools.len(),
            registered_pools.stable_pools.len(),
        );

        Ok(registered_pools)
    }
}

impl BalancerRegisteredPools {
    fn from_graph_pool_data(
        deployment: &PoolFactoryDeployment,
        RegisteredPools {
            fetched_block_number,
            pools,
        }: RegisteredPools,
    ) -> Result<Self> {
        let mut result = Self {
            fetched_block_number,
            ..Default::default()
        };

        for pool in pools {
            match pool.pool_type {
                PoolType::Weighted if pool.factory == deployment.weighted_factory => {
                    result
                        .weighted_pools
                        .push(RegisteredWeightedPool::from_graph_data(
                            &pool,
                            fetched_block_number,
                        )?);
                }
                PoolType::Weighted if pool.factory == deployment.weighted_2token_factory => {
                    result
                        .weighted_2token_pools
                        .push(RegisteredWeightedPool::from_graph_data(
                            &pool,
                            fetched_block_number,
                        )?);
                }
                PoolType::Stable if pool.factory == deployment.stable_factory => {
                    result
                        .stable_pools
                        .push(RegisteredStablePool::from_graph_data(
                            &pool,
                            fetched_block_number,
                        )?);
                }
                _ => {
                    // Log an error in order to trigger an alert. This will
                    // allow us to make sure we get notified if new pool
                    // factories are added that we don't index for.
                    tracing::error!(
                        "unsupported {:?} pool factory {:?}",
                        pool.pool_type,
                        pool.factory
                    );
                }
            }
        }

        Ok(result)
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
trait BalancerSubgraph: Send + Sync {
    async fn registered_pools(&self) -> Result<RegisteredPools>;
}

#[async_trait::async_trait]
impl BalancerSubgraph for BalancerSubgraphClient {
    async fn registered_pools(&self) -> Result<RegisteredPools> {
        self.get_registered_pools().await
    }
}

#[cfg_attr(test, derive(Default))]
struct PoolFactoryDeployment {
    weighted_factory: H160,
    weighted_2token_factory: H160,
    stable_factory: H160,
}

impl PoolFactoryDeployment {
    fn for_chain(chain_id: u64) -> Result<Self> {
        Ok(Self {
            weighted_factory: deployment_address(
                BalancerV2WeightedPoolFactory::raw_contract(),
                chain_id,
            )?,
            weighted_2token_factory: deployment_address(
                BalancerV2WeightedPool2TokensFactory::raw_contract(),
                chain_id,
            )?,
            stable_factory: deployment_address(
                BalancerV2StablePoolFactory::raw_contract(),
                chain_id,
            )?,
        })
    }
}

fn deployment(contract: &Contract, chain_id: u64) -> Result<&Network> {
    contract
        .networks
        .get(&chain_id.to_string())
        // Note that we are conflating network IDs with chain IDs. In general
        // they cannot be considered the same, but for the networks that we
        // support (xDAI, Rinkeby and Mainnet) they are.
        .ok_or_else(|| anyhow!("missing {} deployment for {}", contract.name, chain_id))
}

fn deployment_address(contract: &Contract, chain_id: u64) -> Result<H160> {
    Ok(deployment(contract, chain_id)?.address)
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
    use crate::sources::balancer_v2::{
        graph_api::{PoolData, PoolType, Token},
        pool_storage::{common_pool, CommonPoolData},
        swap::fixed_point::Bfp,
    };
    use anyhow::bail;
    use ethcontract::H256;

    #[tokio::test]
    async fn initializes_empty_pools() {
        let initializer = EmptyPoolInitializer(4);
        assert_eq!(
            initializer.initialize_pools().await.unwrap(),
            BalancerRegisteredPools {
                fetched_block_number: 8441702,
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn empty_initializer_errors_on_missing_deployment() {
        let initializer = EmptyPoolInitializer(999);
        assert!(initializer.initialize_pools().await.is_err());
    }

    #[tokio::test]
    async fn filters_pools_by_factory() {
        let deployment = PoolFactoryDeployment {
            weighted_factory: H160([0xf0; 20]),
            weighted_2token_factory: H160([0xf1; 20]),
            stable_factory: H160([0xf2; 20]),
        };

        fn pool_data(pool_type: PoolType, factory: H160, seed: u8) -> PoolData {
            PoolData {
                pool_type,
                id: H256([seed; 32]),
                address: H160([seed; 20]),
                factory,
                tokens: vec![
                    Token {
                        address: H160([seed; 20]),
                        decimals: 18,
                        weight: Some(Bfp::from_wei(500_000_000_000_000_000u128.into())),
                    },
                    Token {
                        address: H160([seed + 1; 20]),
                        decimals: 18,
                        weight: Some(Bfp::from_wei(500_000_000_000_000_000u128.into())),
                    },
                ],
            }
        }

        let fetched_block_number = 42;
        let weighted_pool = |seed: u8| RegisteredWeightedPool {
            common: CommonPoolData {
                block_created: fetched_block_number,
                ..common_pool(seed)
            },
            weights: vec![
                Bfp::from_wei(500_000_000_000_000_000u128.into()),
                Bfp::from_wei(500_000_000_000_000_000u128.into()),
            ],
        };
        let stable_pool = |seed| RegisteredStablePool {
            common: CommonPoolData {
                block_created: fetched_block_number,
                ..common_pool(seed)
            },
        };

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_registered_pools().returning(move || {
            Ok(RegisteredPools {
                fetched_block_number,
                pools: vec![
                    pool_data(PoolType::Weighted, deployment.weighted_factory, 1),
                    pool_data(PoolType::Weighted, deployment.weighted_2token_factory, 2),
                    pool_data(PoolType::Stable, deployment.stable_factory, 3),
                    pool_data(PoolType::Weighted, deployment.weighted_factory, 4),
                    // Ignores pools from unknown factories
                    pool_data(PoolType::Weighted, H160([0xff; 20]), 5),
                    // Ignores pools from incorrect factories
                    pool_data(PoolType::Stable, deployment.weighted_factory, 6),
                ],
            })
        });

        let initializer = SubgraphPoolInitializer {
            deployment,
            client: Box::new(subgraph),
        };

        assert_eq!(
            initializer.initialize_pools().await.unwrap(),
            BalancerRegisteredPools {
                weighted_pools: vec![weighted_pool(1), weighted_pool(4)],
                weighted_2token_pools: vec![weighted_pool(2)],
                stable_pools: vec![stable_pool(3)],
                fetched_block_number: 42,
            },
        );
    }

    #[tokio::test]
    async fn supports_empty_pools() {
        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_registered_pools().returning(move || {
            Ok(RegisteredPools {
                fetched_block_number: 0,
                pools: vec![],
            })
        });

        let initializer = SubgraphPoolInitializer {
            deployment: PoolFactoryDeployment::default(),
            client: Box::new(subgraph),
        };

        assert_eq!(
            initializer.initialize_pools().await.unwrap(),
            BalancerRegisteredPools::default(),
        );
    }

    #[tokio::test]
    async fn errors_on_subgraph_error() {
        let mut subgraph = MockBalancerSubgraph::new();
        subgraph
            .expect_registered_pools()
            .returning(move || bail!("test error"));

        let initializer = SubgraphPoolInitializer {
            deployment: Default::default(),
            client: Box::new(subgraph),
        };

        assert!(initializer.initialize_pools().await.is_err());
    }

    #[test]
    fn errors_on_missing_deployment() {
        let chain_id = 999;
        assert!(PoolFactoryDeployment::for_chain(chain_id).is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_subgraph_initialization() {
        for (network_name, chain_id) in [("Mainnet", 1), ("Rinkeby", 4)] {
            println!("### {}", network_name);

            let client = SubgraphPoolInitializer::new(chain_id, Client::new()).unwrap();
            let pools = client.initialize_pools().await.unwrap();

            println!(
                "Retrieved {} total pools at block {}",
                pools.weighted_pools.len()
                    + pools.weighted_2token_pools.len()
                    + pools.stable_pools.len(),
                pools.fetched_block_number,
            );
            println!("- {} weighted pools", pools.weighted_pools.len());
            println!(
                "- {} weighted two-token pools",
                pools.weighted_2token_pools.len(),
            );
            println!("- {} stable pools", pools.stable_pools.len());
        }
    }
}
