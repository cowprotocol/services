//! Balancer pool registry initialization.
//!
//! This module contains a component used to initialize Balancer pool registries
//! with existing data in order to reduce the "cold start" time of the service.

use crate::sources::balancer::{
    graph_api::{BalancerSubgraphClient, RegisteredWeightedPools},
    info_fetching::PoolInfoFetching,
    pool_storage::RegisteredWeightedPool,
};
use anyhow::{anyhow, bail, Result};
use contracts::{
    BalancerV2Vault, BalancerV2WeightedPool2TokensFactory, BalancerV2WeightedPoolFactory,
};
use ethcontract::{
    common::{truffle::Network, DeploymentInformation},
    Artifact, H160,
};
use futures::stream::{self, StreamExt as _, TryStreamExt as _};
use std::sync::Arc;

#[derive(Debug, Default, PartialEq)]
pub struct BalancerRegisteredPools {
    pub weighted_pools: Vec<RegisteredWeightedPool>,
    pub weighted_2token_pools: Vec<RegisteredWeightedPool>,
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

#[async_trait::async_trait]
impl PoolInitializing for EmptyPoolInitializer {
    async fn initialize_pools(&self) -> Result<BalancerRegisteredPools> {
        let fetched_block_number = deployment_block(BalancerV2Vault::artifact(), self.0).await?;
        Ok(BalancerRegisteredPools {
            fetched_block_number,
            ..Default::default()
        })
    }
}

/// The default Balancer pool initializer.
pub enum DefaultPoolInitializer {
    Subgraph(SubgraphPoolInitializer),
    Fetched(FetchedPoolInitializer),
}

impl DefaultPoolInitializer {
    pub fn new(chain_id: u64, pool_info: Arc<dyn PoolInfoFetching>) -> Result<Self> {
        const MAINNET_CHAIN_ID: u64 = 1;

        Ok(if chain_id == MAINNET_CHAIN_ID {
            DefaultPoolInitializer::Subgraph(SubgraphPoolInitializer::new(chain_id)?)
        } else {
            // Balancer subgraph seems to only correctly index pool info on
            // chains where it supports archive nodes (because of the required
            // `eth_call`s). This means we can only use the pure Subgraph
            // initializer on Mainnet - the only network with archive node
            // support at the moment.
            DefaultPoolInitializer::Fetched(FetchedPoolInitializer::new(chain_id, pool_info)?)
        })
    }
}

#[async_trait::async_trait]
impl PoolInitializing for DefaultPoolInitializer {
    async fn initialize_pools(&self) -> Result<BalancerRegisteredPools> {
        let registered_pools = match self {
            DefaultPoolInitializer::Subgraph(inner) => inner.initialize_pools().await,
            DefaultPoolInitializer::Fetched(inner) => inner.initialize_pools().await,
        }?;

        tracing::debug!("initialized registered pools {:?}", registered_pools);
        Ok(registered_pools)
    }
}

/// A pool initializer that uses the Balancer subgraph.
pub struct SubgraphPoolInitializer(SubgraphPoolInitializerInner<BalancerSubgraphClient>);

impl SubgraphPoolInitializer {
    pub fn new(chain_id: u64) -> Result<Self> {
        Ok(Self(SubgraphPoolInitializerInner {
            chain_id,
            client: BalancerSubgraphClient::for_chain(chain_id)?,
        }))
    }
}

#[async_trait::async_trait]
impl PoolInitializing for SubgraphPoolInitializer {
    async fn initialize_pools(&self) -> Result<BalancerRegisteredPools> {
        self.0.initialize_pools_inner().await
    }
}

/// Inner generic subgraph pool initializer implementation to allow for mocking
/// and unit tests.
struct SubgraphPoolInitializerInner<S> {
    chain_id: u64,
    client: S,
}

impl<S> SubgraphPoolInitializerInner<S>
where
    S: BalancerSubgraph,
{
    async fn initialize_pools_inner(&self) -> Result<BalancerRegisteredPools> {
        let mut pools = self.client.weighted_pools().await?;
        let result = BalancerRegisteredPools {
            weighted_pools: pools
                .pools_by_factory
                .remove(&deployment_address(
                    BalancerV2WeightedPoolFactory::artifact(),
                    self.chain_id,
                )?)
                .unwrap_or_default(),
            weighted_2token_pools: pools
                .pools_by_factory
                .remove(&deployment_address(
                    BalancerV2WeightedPool2TokensFactory::artifact(),
                    self.chain_id,
                )?)
                .unwrap_or_default(),
            fetched_block_number: pools.fetched_block_number,
        };

        // Log an error in order to trigger an alert. This will allow us to make
        // sure we get notified if new pool factories are added that we don't
        // index for.
        for factory in pools.pools_by_factory.keys() {
            tracing::error!("unsupported pool factory {:?}", factory);
        }

        Ok(result)
    }
}

/// A pool initializer that uses the Balancer subgraph to get all created pool
/// addresses and then fetches pool data onchain.
///
/// This is used for networks such as Rinkeby where the subgraph does not
/// correctly index pool data.
pub struct FetchedPoolInitializer(FetchedPoolInitializerInner<BalancerSubgraphClient>);

impl FetchedPoolInitializer {
    pub fn new(chain_id: u64, pool_info: Arc<dyn PoolInfoFetching>) -> Result<Self> {
        Ok(Self(FetchedPoolInitializerInner {
            chain_id,
            pool_info,
            client: BalancerSubgraphClient::for_chain(chain_id)?,
        }))
    }
}

#[async_trait::async_trait]
impl PoolInitializing for FetchedPoolInitializer {
    async fn initialize_pools(&self) -> Result<BalancerRegisteredPools> {
        self.0.initialize_pools_inner().await
    }
}

/// Inner generic subgraph pool initializer implementation to allow for mocking
/// and unit tests.
struct FetchedPoolInitializerInner<S> {
    chain_id: u64,
    pool_info: Arc<dyn PoolInfoFetching>,
    client: S,
}

impl<S> FetchedPoolInitializerInner<S>
where
    S: BalancerSubgraph,
{
    async fn initialize_pools_inner(&self) -> Result<BalancerRegisteredPools> {
        let mut pools = self.client.weighted_pools().await?;

        // For subgraphs on networks without an archive node (all the testnets)
        // the results from the query will all have missing token data, so fetch
        // them on-chain based on address.

        #[allow(clippy::eval_order_dependence)]
        let result = BalancerRegisteredPools {
            weighted_pools: self
                .fetch_pool_info(
                    pools
                        .pools_by_factory
                        .remove(&deployment_address(
                            BalancerV2WeightedPoolFactory::artifact(),
                            self.chain_id,
                        )?)
                        .unwrap_or_default(),
                    pools.fetched_block_number,
                )
                .await?,
            weighted_2token_pools: self
                .fetch_pool_info(
                    pools
                        .pools_by_factory
                        .remove(&deployment_address(
                            BalancerV2WeightedPool2TokensFactory::artifact(),
                            self.chain_id,
                        )?)
                        .unwrap_or_default(),
                    pools.fetched_block_number,
                )
                .await?,
            fetched_block_number: pools.fetched_block_number,
        };

        // Log an error in order to trigger an alert. This will allow us to make
        // sure we get notified if new pool factories are added that we don't
        // index for.
        for factory in pools.pools_by_factory.keys() {
            tracing::error!("unsupported pool factory {:?}", factory);
        }

        Ok(result)
    }

    async fn fetch_pool_info(
        &self,
        pools: Vec<RegisteredWeightedPool>,
        block_number: u64,
    ) -> Result<Vec<RegisteredWeightedPool>> {
        stream::iter(pools)
            .then(|pool| {
                let pool_info = self.pool_info.clone();
                async move {
                    RegisteredWeightedPool::new(block_number, pool.pool_address, &*pool_info).await
                }
            })
            .try_collect()
            .await
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
trait BalancerSubgraph: Send + Sync {
    async fn weighted_pools(&self) -> Result<RegisteredWeightedPools>;
}

#[async_trait::async_trait]
impl BalancerSubgraph for BalancerSubgraphClient {
    async fn weighted_pools(&self) -> Result<RegisteredWeightedPools> {
        self.get_weighted_pools().await
    }
}

fn deployment(artifact: &Artifact, chain_id: u64) -> Result<&Network> {
    artifact
        .networks
        .get(&chain_id.to_string())
        // Note that we are conflating network IDs with chain IDs. In general
        // they cannot be considered the same, but for the networks that we
        // support (xDAI, Rinkeby and Mainnet) they are.
        .ok_or_else(|| {
            anyhow!(
                "missing {} deployment for {}",
                artifact.contract_name,
                chain_id,
            )
        })
}

fn deployment_address(artifact: &Artifact, chain_id: u64) -> Result<H160> {
    Ok(deployment(artifact, chain_id)?.address)
}

async fn deployment_block(artifact: &Artifact, chain_id: u64) -> Result<u64> {
    let deployment_info = deployment(artifact, chain_id)?
        .deployment_information
        .ok_or_else(|| {
            anyhow!(
                "missing deployment information for {}",
                artifact.contract_name
            )
        })?;

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
    use crate::sources::balancer::{
        info_fetching::{MockPoolInfoFetching, WeightedPoolInfo},
        swap::fixed_point::Bfp,
    };
    use anyhow::bail;
    use ethcontract::H256;
    use maplit::hashmap;
    use mockall::{predicate::*, Sequence};

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
        let chain_id = 1;

        let weighted_factory =
            deployment_address(BalancerV2WeightedPoolFactory::artifact(), chain_id).unwrap();
        let weighted_2token_factory =
            deployment_address(BalancerV2WeightedPool2TokensFactory::artifact(), chain_id).unwrap();

        fn pool(seed: u8) -> RegisteredWeightedPool {
            RegisteredWeightedPool {
                pool_id: H256([seed; 32]),
                pool_address: H160([seed; 20]),
                tokens: vec![H160([seed; 20]), H160([seed + 1; 20])],
                scaling_exponents: vec![0, 0],
                normalized_weights: vec![
                    Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    Bfp::from_wei(500_000_000_000_000_000u128.into()),
                ],
                block_created: seed as _,
            }
        }

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_weighted_pools().returning(move || {
            Ok(RegisteredWeightedPools {
                pools_by_factory: hashmap! {
                    weighted_factory => vec![
                        pool(1),
                        pool(2),
                    ],
                    weighted_2token_factory => vec![
                        pool(3),
                    ],
                    addr!("0102030405060708091011121314151617181920") => vec![
                        pool(4),
                        pool(5),
                    ],
                },
                fetched_block_number: 42,
            })
        });

        let initializer = SubgraphPoolInitializerInner {
            chain_id,
            client: subgraph,
        };

        assert_eq!(
            initializer.initialize_pools_inner().await.unwrap(),
            BalancerRegisteredPools {
                weighted_pools: vec![pool(1), pool(2)],
                weighted_2token_pools: vec![pool(3)],
                fetched_block_number: 42,
            },
        );
    }

    #[tokio::test]
    async fn supports_empty_and_missing_factories() {
        let chain_id = 4;

        let weighted_2token_factory =
            deployment_address(BalancerV2WeightedPool2TokensFactory::artifact(), chain_id).unwrap();

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_weighted_pools().returning(move || {
            Ok(RegisteredWeightedPools {
                pools_by_factory: hashmap! {
                    weighted_2token_factory => vec![],
                },
                fetched_block_number: 0,
            })
        });

        let initializer = SubgraphPoolInitializerInner {
            chain_id,
            client: subgraph,
        };

        assert_eq!(
            initializer.initialize_pools_inner().await.unwrap(),
            BalancerRegisteredPools::default(),
        );
    }

    #[tokio::test]
    async fn errors_on_subgraph_error() {
        let chain_id = 1;

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph
            .expect_weighted_pools()
            .returning(move || bail!("test error"));

        let initializer = SubgraphPoolInitializerInner {
            chain_id,
            client: subgraph,
        };

        assert!(initializer.initialize_pools_inner().await.is_err());
    }

    #[tokio::test]
    async fn errors_on_missing_deployment() {
        let chain_id = 999;

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_weighted_pools().returning(|| {
            Ok(RegisteredWeightedPools {
                pools_by_factory: hashmap! {},
                fetched_block_number: 0,
            })
        });

        let initializer = SubgraphPoolInitializerInner {
            chain_id,
            client: subgraph,
        };

        assert!(initializer.initialize_pools_inner().await.is_err());
    }

    #[tokio::test]
    async fn fetches_pool_info_on_chain() {
        let chain_id = 1;

        let weighted_factory =
            deployment_address(BalancerV2WeightedPoolFactory::artifact(), chain_id).unwrap();
        let weighted_2token_factory =
            deployment_address(BalancerV2WeightedPool2TokensFactory::artifact(), chain_id).unwrap();

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_weighted_pools().returning(move || {
            Ok(RegisteredWeightedPools {
                pools_by_factory: hashmap! {
                    weighted_factory => vec![RegisteredWeightedPool {
                        pool_id: H256([1; 32]),
                        pool_address: H160([1; 20]),
                        tokens: vec![],
                        scaling_exponents: vec![],
                        normalized_weights: vec![],
                        block_created: 42,
                    }],
                    weighted_2token_factory => vec![RegisteredWeightedPool {
                        pool_id: H256([2; 32]),
                        pool_address: H160([2; 20]),
                        tokens: vec![],
                        scaling_exponents: vec![],
                        normalized_weights: vec![],
                        block_created: 42,
                    }],
                    addr!("0102030405060708091011121314151617181920") => vec![RegisteredWeightedPool {
                        pool_id: H256([3; 32]),
                        pool_address: H160([3; 20]),
                        tokens: vec![],
                        scaling_exponents: vec![],
                        normalized_weights: vec![],
                        block_created: 42,
                    }],
                },
                fetched_block_number: 42,
            })
        });

        let mut pool_info = MockPoolInfoFetching::new();
        let mut seq = Sequence::new();
        pool_info
            .expect_get_pool_data()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(H160([1; 20])))
            .returning(|_| {
                Ok(WeightedPoolInfo {
                    pool_id: H256([1; 32]),
                    tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                    scaling_exponents: vec![0, 0],
                    weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                })
            });
        pool_info
            .expect_get_pool_data()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(H160([2; 20])))
            .returning(|_| {
                Ok(WeightedPoolInfo {
                    pool_id: H256([2; 32]),
                    tokens: vec![H160([0x11; 20]), H160([0x33; 20]), H160([0x44; 20])],
                    scaling_exponents: vec![0, 0, 0],
                    weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                    ],
                })
            });

        let initializer = FetchedPoolInitializerInner {
            chain_id,
            pool_info: Arc::new(pool_info),
            client: subgraph,
        };

        assert_eq!(
            initializer.initialize_pools_inner().await.unwrap(),
            BalancerRegisteredPools {
                weighted_pools: vec![RegisteredWeightedPool {
                    pool_id: H256([1; 32]),
                    pool_address: H160([1; 20]),
                    tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                    scaling_exponents: vec![0, 0],
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                    block_created: 42,
                }],
                weighted_2token_pools: vec![RegisteredWeightedPool {
                    pool_id: H256([2; 32]),
                    pool_address: H160([2; 20]),
                    tokens: vec![H160([0x11; 20]), H160([0x33; 20]), H160([0x44; 20])],
                    scaling_exponents: vec![0, 0, 0],
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                    ],
                    block_created: 42,
                }],
                fetched_block_number: 42,
            },
        );
    }
}
