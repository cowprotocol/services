//! Module containing The Graph API client used for retrieving Balancer weighted
//! pools from the Balancer V2 subgraph.
//!
//! The pools retrieved from this client are used to prime the graph event store
//! to reduce start-up time. We do not use this in general for retrieving pools
//! as to:
//! - not rely on external services
//! - ensure that we are using the latest up-to-date pool data by using events
//!   from the node

use super::{
    pool_storage::{CommonPoolData, RegisteredStablePool, RegisteredWeightedPool},
    swap::fixed_point::Bfp,
};
use crate::{event_handling::MAX_REORG_BLOCK_COUNT, subgraph::SubgraphClient};
use anyhow::{anyhow, bail, Result};
use ethcontract::{H160, H256};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use serde_with::{serde_as, DisplayFromStr};

/// The page size when querying pools.
#[cfg(not(test))]
const QUERY_PAGE_SIZE: usize = 1000;
#[cfg(test)]
const QUERY_PAGE_SIZE: usize = 10;

/// A client to the Balancer V2 subgraph.
///
/// This client is not implemented to allow general GraphQL queries, but instead
/// implements high-level methods that perform GraphQL queries under the hood.
pub struct BalancerSubgraphClient(SubgraphClient);

impl BalancerSubgraphClient {
    /// Creates a new Balancer subgraph client for the specified chain ID.
    pub fn for_chain(chain_id: u64, client: Client) -> Result<Self> {
        let subgraph_name = match chain_id {
            1 => "balancer-v2",
            4 => "balancer-rinkeby-v2",
            _ => bail!("unsupported chain {}", chain_id),
        };
        Ok(Self(SubgraphClient::new(
            "balancer-labs",
            subgraph_name,
            client,
        )?))
    }

    /// Retrieves the list of registered pools from the subgraph.
    pub async fn get_registered_pools(&self) -> Result<RegisteredPools> {
        use self::pools_query::*;

        let block_number = self.get_safe_block().await?;

        let mut pools = Vec::new();
        let mut last_id = H256::default();

        // We do paging by last ID instead of using `skip`. This is the
        // suggested approach to paging best performance:
        // <https://thegraph.com/docs/graphql-api#pagination>
        loop {
            let page = self
                .0
                .query::<Data>(
                    QUERY,
                    Some(json_map! {
                        "block" => block_number,
                        "pageSize" => QUERY_PAGE_SIZE,
                        "lastId" => json!(last_id),
                    }),
                )
                .await?
                .pools;
            let no_more_pages = page.len() != QUERY_PAGE_SIZE;
            if let Some(last_pool) = page.last() {
                last_id = last_pool.id;
            }

            pools.extend(page);

            if no_more_pages {
                break;
            }
        }

        Ok(RegisteredPools {
            fetched_block_number: block_number,
            pools,
        })
    }

    /// Retrieves a recent block number for which it is safe to assume no
    /// reorgs will happen.
    async fn get_safe_block(&self) -> Result<u64> {
        // Ideally we would want to use block hash here so that we can check
        // that there indeed is no reorg. However, it does not seem possible to
        // retrieve historic block hashes just from the subgraph (it always
        // returns `null`).
        Ok(self
            .0
            .query::<block_number_query::Data>(block_number_query::QUERY, None)
            .await?
            .meta
            .block
            .number
            .saturating_sub(MAX_REORG_BLOCK_COUNT))
    }
}

/// Result of the registered stable pool query.
#[derive(Debug, PartialEq)]
pub struct RegisteredPools {
    /// The block number that the data was fetched, and for which the registered
    /// weighted pools can be considered up to date.
    pub fetched_block_number: u64,
    /// The registered Pools
    pub pools: Vec<PoolData>,
}

/// Pool data from the Balancer V2 subgraph.
#[derive(Debug, Deserialize, PartialEq)]
pub struct PoolData {
    #[serde(rename = "poolType")]
    pub pool_type: PoolType,
    pub id: H256,
    pub address: H160,
    pub factory: H160,
    pub tokens: Vec<Token>,
}

/// Supported pool kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
pub enum PoolType {
    Stable,
    Weighted,
}

/// Token data for pools.
#[serde_as]
#[derive(Debug, Deserialize, PartialEq)]
pub struct Token {
    pub address: H160,
    pub decimals: u8,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub weight: Option<Bfp>,
}

impl PoolData {
    /// Returns the Balancer subgraph pool data as an internal representation of
    /// common pool data shared accross all Balancer pools.
    fn as_common_pool_data(&self, block_created: u64) -> Result<CommonPoolData> {
        Ok(CommonPoolData {
            pool_id: self.id,
            pool_address: self.address,
            tokens: self.tokens.iter().map(|token| token.address).collect(),
            scaling_exponents: self
                .tokens
                .iter()
                .map(|token| scaling_exponent_from_decimals(token.decimals))
                .collect::<Result<_>>()?,
            block_created,
        })
    }

    /// Returns the Balancer subgraph pool data as the internal representation
    /// of a Balancer weighted pool.
    pub fn as_weighted(&self, block_created: u64) -> Result<RegisteredWeightedPool> {
        Ok(RegisteredWeightedPool {
            common: self.as_common_pool_data(block_created)?,
            normalized_weights: self
                .tokens
                .iter()
                .map(|token| {
                    token
                        .weight
                        .ok_or_else(|| anyhow!("missing weights for pool {:?}", self.id))
                })
                .collect::<Result<_>>()?,
        })
    }

    /// Returns the Balancer subgraph pool data as the internal representation
    /// of a Balancer stable pool.
    pub fn as_stable(&self, block_created: u64) -> Result<RegisteredStablePool> {
        Ok(RegisteredStablePool {
            common: self.as_common_pool_data(block_created)?,
        })
    }
}

fn scaling_exponent_from_decimals(decimals: u8) -> Result<u8> {
    // Technically this should never fail for Balancer Pools since tokens
    // with more than 18 decimals (not supported by balancer contracts)
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/deployments-latest/pkg/pool-utils/contracts/BasePool.sol#L476-L487
    18u8.checked_sub(decimals)
        .ok_or_else(|| anyhow!("unsupported token with more than 18 decimals"))
}

mod pools_query {
    use super::PoolData;
    use serde::Deserialize;

    pub const QUERY: &str = r#"
        query Pools($block: Int, $pageSize: Int, $lastId: ID) {
            pools(
                block: { number: $block }
                first: $pageSize
                where: {
                    id_gt: $lastId
                    poolType_in: ["Stable","Weighted"]
                }
            ) {
                poolType
                id
                address
                factory
                tokens {
                    address
                    decimals
                    weight
                }
            }
        }
    "#;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Data {
        pub pools: Vec<PoolData>,
    }
}

mod block_number_query {
    use serde::Deserialize;

    pub const QUERY: &str = r#"{
        _meta {
            block { number }
        }
    }"#;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Data {
        #[serde(rename = "_meta")]
        pub meta: Meta,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Meta {
        pub block: Block,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Block {
        pub number: u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::{
        pool_storage::{CommonPoolData, RegisteredStablePool, RegisteredWeightedPool},
        swap::fixed_point::Bfp,
    };
    use ethcontract::{H160, H256};
    use std::collections::HashMap;

    #[test]
    fn decode_pools_data() {
        use pools_query::*;

        assert_eq!(
            serde_json::from_value::<Data>(json!({
                "pools": [
                    {
                        "poolType": "Weighted",
                        "address": "0x2222222222222222222222222222222222222222",
                        "id": "0x1111111111111111111111111111111111111111111111111111111111111111",
                        "factory": "0x5555555555555555555555555555555555555555",
                        "tokens": [
                            {
                                "address": "0x3333333333333333333333333333333333333333",
                                "decimals": 3,
                                "weight": "0.5"
                            },
                            {
                                "address": "0x4444444444444444444444444444444444444444",
                                "decimals": 4,
                                "weight": "0.5"
                            },
                        ],
                    },
                    {
                        "poolType": "Stable",
                        "address": "0x2222222222222222222222222222222222222222",
                        "id": "0x1111111111111111111111111111111111111111111111111111111111111111",
                        "factory": "0x5555555555555555555555555555555555555555",
                        "tokens": [
                            {
                                "address": "0x3333333333333333333333333333333333333333",
                                "decimals": 3,
                            },
                            {
                                "address": "0x4444444444444444444444444444444444444444",
                                "decimals": 4,
                            },
                        ],
                    },
                ],
            }))
            .unwrap(),
            Data {
                pools: vec![
                    PoolData {
                        pool_type: PoolType::Weighted,
                        id: H256([0x11; 32]),
                        address: H160([0x22; 20]),
                        factory: H160([0x55; 20]),
                        tokens: vec![
                            Token {
                                address: H160([0x33; 20]),
                                decimals: 3,
                                weight: Some(Bfp::from_wei(500_000_000_000_000_000u128.into())),
                            },
                            Token {
                                address: H160([0x44; 20]),
                                decimals: 4,
                                weight: Some(Bfp::from_wei(500_000_000_000_000_000u128.into())),
                            },
                        ],
                    },
                    PoolData {
                        pool_type: PoolType::Stable,
                        id: H256([0x11; 32]),
                        address: H160([0x22; 20]),
                        factory: H160([0x55; 20]),
                        tokens: vec![
                            Token {
                                address: H160([0x33; 20]),
                                decimals: 3,
                                weight: None,
                            },
                            Token {
                                address: H160([0x44; 20]),
                                decimals: 4,
                                weight: None,
                            },
                        ],
                    }
                ],
            }
        );
    }

    #[test]
    fn decode_block_number_data() {
        use block_number_query::*;

        assert_eq!(
            serde_json::from_value::<Data>(json!({
                "_meta": {
                    "block": {
                        "number": 42,
                    },
                },
            }))
            .unwrap(),
            Data {
                meta: Meta {
                    block: Block { number: 42 }
                }
            }
        );
    }

    #[test]
    fn convert_pool_to_registered_weighted_pool() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0xfa; 20]),
            tokens: vec![
                Token {
                    address: H160([0x11; 20]),
                    decimals: 1,
                    weight: Some("1.337".parse().unwrap()),
                },
                Token {
                    address: H160([0x22; 20]),
                    decimals: 2,
                    weight: Some("4.2".parse().unwrap()),
                },
            ],
        };

        assert_eq!(
            pool.as_weighted(42).unwrap(),
            RegisteredWeightedPool {
                common: CommonPoolData {
                    pool_id: H256([2; 32]),
                    pool_address: H160([1; 20]),
                    tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                    scaling_exponents: vec![17, 16],
                    block_created: 42,
                },
                normalized_weights: vec![
                    Bfp::from_wei(1_337_000_000_000_000_000u128.into()),
                    Bfp::from_wei(4_200_000_000_000_000_000u128.into()),
                ],
            },
        );
    }

    #[test]
    fn convert_pool_to_registered_stable_pool() {
        let pool = PoolData {
            pool_type: PoolType::Stable,
            id: H256([4; 32]),
            address: H160([3; 20]),
            factory: H160([0xfb; 20]),
            tokens: vec![
                Token {
                    address: H160([0x33; 20]),
                    decimals: 3,
                    weight: None,
                },
                Token {
                    address: H160([0x44; 20]),
                    decimals: 18,
                    weight: None,
                },
            ],
        };

        assert_eq!(
            pool.as_stable(42).unwrap(),
            RegisteredStablePool {
                common: CommonPoolData {
                    pool_id: H256([4; 32]),
                    pool_address: H160([3; 20]),
                    tokens: vec![H160([0x33; 20]), H160([0x44; 20])],
                    scaling_exponents: vec![15, 0],
                    block_created: 42,
                },
            }
        );
    }

    #[test]
    fn pool_conversion_invalid_decimals() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0; 20]),
            tokens: vec![Token {
                address: H160([2; 20]),
                decimals: 19,
                weight: Some("1.337".parse().unwrap()),
            }],
        };
        assert!(pool.as_common_pool_data(42).is_err());
    }

    #[test]
    fn scaling_exponent_from_decimals_ok_and_err() {
        for i in 0..=18 {
            assert_eq!(scaling_exponent_from_decimals(i).unwrap(), 18u8 - i);
        }
        assert_eq!(
            scaling_exponent_from_decimals(19).unwrap_err().to_string(),
            "unsupported token with more than 18 decimals"
        )
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_subgraph_query() {
        for (network_name, chain_id) in [("Mainnet", 1), ("Rinkeby", 4)] {
            println!("### {}", network_name);

            let client = BalancerSubgraphClient::for_chain(chain_id, Client::new()).unwrap();
            let result = client.get_registered_pools().await.unwrap();
            println!(
                "Retrieved {} total pools at block {}",
                result.pools.len(),
                result.fetched_block_number,
            );

            let grouped_by_factory = result.pools.into_iter().fold(
                HashMap::<_, Vec<_>>::new(),
                |mut factories, pool| {
                    factories
                        .entry((pool.pool_type, pool.factory))
                        .or_default()
                        .push(pool);
                    factories
                },
            );
            for ((pool_type, factory), pools) in grouped_by_factory {
                println!(
                    "- {} {:?} pools from factory {:?}",
                    pools.len(),
                    pool_type,
                    factory,
                );
            }
        }
    }
}
