//! Balancer pool registry initialization.
//!
//! This module contains a component used to initialize Balancer pool registries
//! with existing data in order to reduce the "cold start" time of the service.

use crate::sources::balancer_v2::{
    graph_api::{BalancerSubgraphClient, RegisteredPools},
    info_fetching::PoolInfoFetching,
    pool_storage::{RegisteredStablePool, RegisteredWeightedPool},
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
use futures::stream::{self, StreamExt as _, TryStreamExt as _};
use reqwest::Client;
use std::sync::Arc;

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

/// The default Balancer pool initializer.
pub enum DefaultPoolInitializer {
    Subgraph(SubgraphPoolInitializer),
    Fetched(FetchedPoolInitializer),
}

impl DefaultPoolInitializer {
    pub fn new(
        chain_id: u64,
        pool_info: Arc<dyn PoolInfoFetching>,
        client: Client,
    ) -> Result<Self> {
        const MAINNET_CHAIN_ID: u64 = 1;

        Ok(if chain_id == MAINNET_CHAIN_ID {
            DefaultPoolInitializer::Subgraph(SubgraphPoolInitializer::new(chain_id, client)?)
        } else {
            // Balancer subgraph seems to only correctly index pool info on
            // chains where it supports archive nodes (because of the required
            // `eth_call`s). This means we can only use the pure Subgraph
            // initializer on Mainnet - the only network with archive node
            // support at the moment.
            DefaultPoolInitializer::Fetched(FetchedPoolInitializer::new(
                chain_id, pool_info, client,
            )?)
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
        tracing::debug!(
            "initialized registered pools ({} Stable, {} Weighted & {} TwoTokenWeighted)",
            registered_pools.stable_pools.len(),
            registered_pools.weighted_pools.len(),
            registered_pools.weighted_2token_pools.len()
        );
        Ok(registered_pools)
    }
}

/// A pool initializer that uses the Balancer subgraph.
pub struct SubgraphPoolInitializer(SubgraphPoolInitializerInner<BalancerSubgraphClient>);

impl SubgraphPoolInitializer {
    pub fn new(chain_id: u64, client: Client) -> Result<Self> {
        Ok(Self(SubgraphPoolInitializerInner {
            chain_id,
            client: BalancerSubgraphClient::for_chain(chain_id, client)?,
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
        let mut pools = self.client.registered_pools().await?;
        let result = BalancerRegisteredPools {
            weighted_pools: pools
                .weighted_pools_by_factory
                .remove(&deployment_address(
                    BalancerV2WeightedPoolFactory::raw_contract(),
                    self.chain_id,
                )?)
                .unwrap_or_default(),
            weighted_2token_pools: pools
                .weighted_pools_by_factory
                .remove(&deployment_address(
                    BalancerV2WeightedPool2TokensFactory::raw_contract(),
                    self.chain_id,
                )?)
                .unwrap_or_default(),
            stable_pools: pools
                .stable_pools_by_factory
                .remove(&deployment_address(
                    BalancerV2StablePoolFactory::raw_contract(),
                    self.chain_id,
                )?)
                .unwrap_or_default(),
            fetched_block_number: pools.fetched_block_number,
        };

        // Log an error in order to trigger an alert. This will allow us to make
        // sure we get notified if new pool factories are added that we don't
        // index for.
        for factory in pools.weighted_pools_by_factory.keys() {
            tracing::error!("unsupported weighted pool factory {:?}", factory);
        }

        for factory in pools.stable_pools_by_factory.keys() {
            tracing::error!("unsupported stable pool factory {:?}", factory);
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
    pub fn new(
        chain_id: u64,
        pool_info: Arc<dyn PoolInfoFetching>,
        client: Client,
    ) -> Result<Self> {
        Ok(Self(FetchedPoolInitializerInner {
            chain_id,
            pool_info,
            client: BalancerSubgraphClient::for_chain(chain_id, client)?,
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
        let mut registered_pools = self.client.registered_pools().await?;

        // For subgraphs on networks without an archive node (all the testnets)
        // the results from the query will all have missing token data, so fetch
        // them on-chain based on address.

        #[allow(clippy::eval_order_dependence)]
        let result = BalancerRegisteredPools {
            weighted_pools: self
                .fetch_weighted_pool_info(
                    registered_pools
                        .weighted_pools_by_factory
                        .remove(&deployment_address(
                            BalancerV2WeightedPoolFactory::raw_contract(),
                            self.chain_id,
                        )?)
                        .unwrap_or_default(),
                    registered_pools.fetched_block_number,
                )
                .await?,
            weighted_2token_pools: self
                .fetch_weighted_pool_info(
                    registered_pools
                        .weighted_pools_by_factory
                        .remove(&deployment_address(
                            BalancerV2WeightedPool2TokensFactory::raw_contract(),
                            self.chain_id,
                        )?)
                        .unwrap_or_default(),
                    registered_pools.fetched_block_number,
                )
                .await?,
            stable_pools: self
                .fetch_stable_pool_info(
                    registered_pools
                        .stable_pools_by_factory
                        .remove(&deployment_address(
                            BalancerV2StablePoolFactory::raw_contract(),
                            self.chain_id,
                        )?)
                        .unwrap_or_default(),
                    registered_pools.fetched_block_number,
                )
                .await?,
            fetched_block_number: registered_pools.fetched_block_number,
        };

        // Log an error in order to trigger an alert. This will allow us to make
        // sure we get notified if new pool factories are added that we don't
        // index for.
        for factory in registered_pools.weighted_pools_by_factory.keys() {
            tracing::error!("unsupported weighted pool factory {:?}", factory);
        }

        for factory in registered_pools.stable_pools_by_factory.keys() {
            tracing::error!("unsupported stable pool factory {:?}", factory);
        }

        Ok(result)
    }

    async fn fetch_weighted_pool_info(
        &self,
        pools: Vec<RegisteredWeightedPool>,
        block_number: u64,
    ) -> Result<Vec<RegisteredWeightedPool>> {
        stream::iter(pools)
            .then(|pool| {
                let pool_info = self.pool_info.clone();
                async move {
                    RegisteredWeightedPool::new(block_number, pool.common.pool_address, &*pool_info)
                        .await
                }
            })
            .try_collect()
            .await
    }

    async fn fetch_stable_pool_info(
        &self,
        pools: Vec<RegisteredStablePool>,
        block_number: u64,
    ) -> Result<Vec<RegisteredStablePool>> {
        stream::iter(pools)
            .then(|pool| {
                let pool_info = self.pool_info.clone();
                async move {
                    RegisteredStablePool::new(block_number, pool.common.pool_address, &*pool_info)
                        .await
                }
            })
            .try_collect()
            .await
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
        info_fetching::{CommonPoolInfo, MockPoolInfoFetching, StablePoolInfo, WeightedPoolInfo},
        pool_storage::{common_pool, CommonPoolData, RegisteredStablePool},
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
            deployment_address(BalancerV2WeightedPoolFactory::raw_contract(), chain_id).unwrap();
        let weighted_2token_factory = deployment_address(
            BalancerV2WeightedPool2TokensFactory::raw_contract(),
            chain_id,
        )
        .unwrap();
        let stable_factory =
            deployment_address(BalancerV2StablePoolFactory::raw_contract(), chain_id).unwrap();

        fn weighted_pool(seed: u8) -> RegisteredWeightedPool {
            RegisteredWeightedPool {
                common: common_pool(seed),
                normalized_weights: vec![
                    Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    Bfp::from_wei(500_000_000_000_000_000u128.into()),
                ],
            }
        }

        fn stable_pool(seed: u8) -> RegisteredStablePool {
            RegisteredStablePool {
                common: common_pool(seed),
            }
        }

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_registered_pools().returning(move || {
            Ok(RegisteredPools {
                weighted_pools_by_factory: hashmap! {
                    weighted_factory => vec![
                        weighted_pool(1),
                        weighted_pool(2),
                    ],
                    weighted_2token_factory => vec![
                        weighted_pool(3),
                    ],
                    addr!("0102030405060708091011121314151617181920") => vec![
                        weighted_pool(4),
                    ],
                },
                stable_pools_by_factory: hashmap! {
                    stable_factory => vec![
                        stable_pool(6),
                    ],
                    addr!("1102030405060708008011121314151617181920") => vec![
                        stable_pool(5),
                    ]
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
                weighted_pools: vec![weighted_pool(1), weighted_pool(2)],
                weighted_2token_pools: vec![weighted_pool(3)],
                stable_pools: vec![stable_pool(6)],
                fetched_block_number: 42,
            },
        );
    }

    #[tokio::test]
    async fn supports_empty_and_missing_factories() {
        let chain_id = 4;

        let weighted_2token_factory = deployment_address(
            BalancerV2WeightedPool2TokensFactory::raw_contract(),
            chain_id,
        )
        .unwrap();

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_registered_pools().returning(move || {
            Ok(RegisteredPools {
                weighted_pools_by_factory: hashmap! {
                    weighted_2token_factory => vec![],
                },
                stable_pools_by_factory: hashmap! {},
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
            .expect_registered_pools()
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
        subgraph.expect_registered_pools().returning(|| {
            Ok(RegisteredPools {
                weighted_pools_by_factory: hashmap! {},
                stable_pools_by_factory: hashmap! {},
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
            deployment_address(BalancerV2WeightedPoolFactory::raw_contract(), chain_id).unwrap();
        let weighted_2token_factory = deployment_address(
            BalancerV2WeightedPool2TokensFactory::raw_contract(),
            chain_id,
        )
        .unwrap();
        let stable_factory =
            deployment_address(BalancerV2StablePoolFactory::raw_contract(), chain_id).unwrap();

        let mut subgraph = MockBalancerSubgraph::new();
        subgraph.expect_registered_pools().returning(move || {
            Ok(RegisteredPools {
                weighted_pools_by_factory: hashmap! {
                    weighted_factory => vec![RegisteredWeightedPool {
                        common: CommonPoolData {
                            pool_id: H256([1; 32]),
                            pool_address: H160([1; 20]),
                            tokens: vec![],
                            scaling_exponents: vec![],
                            block_created: 42,
                        },
                        normalized_weights: vec![],
                    }],
                    weighted_2token_factory => vec![RegisteredWeightedPool {
                        common: CommonPoolData {
                            pool_id: H256([2; 32]),
                            pool_address: H160([2; 20]),
                            tokens: vec![],
                            scaling_exponents: vec![],
                            block_created: 42,
                        },
                        normalized_weights: vec![],
                    }],
                    addr!("0102030405060708091011121314151617181920") => vec![
                        RegisteredWeightedPool {
                            common: CommonPoolData {
                                pool_id: H256([4; 32]),
                                pool_address: H160([4; 20]),
                                tokens: vec![],
                                scaling_exponents: vec![],
                                block_created: 42,
                            },
                            normalized_weights: vec![],
                        },
                    ],
                },
                stable_pools_by_factory: hashmap! {
                    stable_factory => vec![RegisteredStablePool {
                        common: CommonPoolData {
                            pool_id: H256([3; 32]),
                            pool_address: H160([3; 20]),
                            tokens: vec![],
                            scaling_exponents: vec![],
                            block_created: 42,
                        }
                    }],
                    addr!("0102030405060708091011121314151617181920") => vec![
                        RegisteredStablePool {
                            common: CommonPoolData {
                                pool_id: H256([5; 32]),
                                pool_address: H160([5; 20]),
                                tokens: vec![],
                                scaling_exponents: vec![],
                                block_created: 42,
                            }
                        }
                    ],
                },
                fetched_block_number: 42,
            })
        });

        let mut pool_info = MockPoolInfoFetching::new();
        let mut seq = Sequence::new();
        pool_info
            .expect_get_weighted_pool_data()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(H160([1; 20])))
            .returning(|_| {
                Ok(WeightedPoolInfo {
                    common: CommonPoolInfo {
                        pool_id: H256([1; 32]),
                        tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                        scaling_exponents: vec![0, 0],
                    },
                    weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                })
            });
        pool_info
            .expect_get_weighted_pool_data()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(H160([2; 20])))
            .returning(|_| {
                Ok(WeightedPoolInfo {
                    common: CommonPoolInfo {
                        pool_id: H256([2; 32]),
                        tokens: vec![H160([0x11; 20]), H160([0x33; 20]), H160([0x44; 20])],
                        scaling_exponents: vec![0, 0, 0],
                    },
                    weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                    ],
                })
            });
        pool_info
            .expect_get_stable_pool_data()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(H160([3; 20])))
            .returning(|_| {
                Ok(StablePoolInfo {
                    common: CommonPoolInfo {
                        pool_id: H256([3; 32]),
                        tokens: vec![],
                        scaling_exponents: vec![],
                    },
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
                    common: CommonPoolData {
                        pool_id: H256([1; 32]),
                        pool_address: H160([1; 20]),
                        tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                        scaling_exponents: vec![0, 0],
                        block_created: 42,
                    },
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                    ],
                }],
                weighted_2token_pools: vec![RegisteredWeightedPool {
                    common: CommonPoolData {
                        pool_id: H256([2; 32]),
                        pool_address: H160([2; 20]),
                        tokens: vec![H160([0x11; 20]), H160([0x33; 20]), H160([0x44; 20])],
                        scaling_exponents: vec![0, 0, 0],
                        block_created: 42,
                    },
                    normalized_weights: vec![
                        Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                        Bfp::from_wei(250_000_000_000_000_000u128.into()),
                    ],
                }],
                stable_pools: vec![RegisteredStablePool {
                    common: CommonPoolData {
                        pool_id: H256([3; 32]),
                        pool_address: H160([3; 20]),
                        tokens: vec![],
                        scaling_exponents: vec![],
                        block_created: 42,
                    }
                }],
                fetched_block_number: 42,
            },
        );
    }
}
