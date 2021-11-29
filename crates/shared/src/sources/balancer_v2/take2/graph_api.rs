//! Module containing The Graph API client used for retrieving Balancer weighted
//! pools from the Balancer V2 subgraph.
//!
//! The pools retrieved from this client are used to prime the graph event store
//! to reduce start-up time. We do not use this in general for retrieving pools
//! as to not rely on external services.
//!
//! TODO(nlordell): Unified query.

use super::math::fixed_point::Bfp;
use crate::{event_handling::MAX_REORG_BLOCK_COUNT, subgraph::SubgraphClient};
use anyhow::{bail, Result};
use ethcontract::{H160, H256};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use serde_with::{serde_as, DisplayFromStr};
use std::collections::HashMap;

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

        let mut pools_by_factory = HashMap::<_, Vec<_>>::new();
        for pool in pools {
            pools_by_factory
                .entry(pool.address)
                .or_default()
                .push(pool.id);
        }

        Ok(RegisteredPools {
            fetched_block_number: block_number,
            pools_by_factory,
        })
    }

    /// Retrieves a recent block number for which it is safe to assume no
    /// reorgs will happen.
    async fn get_safe_block(&self) -> Result<u64> {
        use self::block_number_query::*;

        // Ideally we would want to use block hash here so that we can check
        // that there indeed is no reorg. However, it does not seem possible to
        // retrieve historic block hashes just from the subgraph (it always
        // returns `null`).
        let data = self.0.query::<Data>(QUERY, None).await?;
        let safe_block = data.meta.block.number.saturating_sub(MAX_REORG_BLOCK_COUNT);
        Ok(safe_block)
    }
}

/// Result of the registered stable pool query.
pub struct RegisteredPools {
    /// The block number that the data was fetched, and for which the registered
    /// weighted pools can be considered up to date.
    pub fetched_block_number: u64,
    /// The registered Pools
    ///
    /// TODO(nlordell): Something of substance.
    pub pools_by_factory: HashMap<H160, Vec<H256>>,
}

/// Pool data retrived from the Graph API.
#[derive(Debug, Deserialize, PartialEq)]
pub struct PoolData {
    #[serde(rename = "poolType")]
    pub kind: PoolKind,
    pub id: H256,
    pub address: H160,
    pub factory: Option<H160>,
    pub tokens: Vec<Token>,
}

/// Supported pool kinds.
#[derive(Debug, Deserialize, PartialEq)]
pub enum PoolKind {
    Stable,
    Weighted,
}

/// Pool tokens with parameters
#[serde_as]
#[derive(Debug, Deserialize, PartialEq)]
pub struct Token {
    pub address: H160,
    pub decimals: u8,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub weight: Option<Bfp>,
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
    /*
    use super::*;
    use crate::sources::balancer_v2::{
        pool_storage::{CommonPoolData, RegisteredStablePool, RegisteredWeightedPool},
        swap::fixed_point::Bfp,
    };
    use ethcontract::{H160, H256};

    #[test]
    fn decode_pools_data() {
        use pools_query::*;

        assert_eq!(
            serde_json::from_value::<Data<WeightedToken>>(json!({
                "pools": [
                    {
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
                ],
            }))
            .unwrap(),
            Data {
                pools: vec![PoolData {
                    id: H256([0x11; 32]),
                    address: H160([0x22; 20]),
                    factory: Some(H160([0x55; 20])),
                    tokens: vec![
                        WeightedToken {
                            address: H160([0x33; 20]),
                            decimals: 3,
                            weight: Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        },
                        WeightedToken {
                            address: H160([0x44; 20]),
                            decimals: 4,
                            weight: Bfp::from_wei(500_000_000_000_000_000u128.into()),
                        },
                    ],
                }],
            }
        );

        assert_eq!(
            serde_json::from_value::<Data<StableToken>>(json!({
                "pools": [
                    {
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
                pools: vec![PoolData {
                    id: H256([0x11; 32]),
                    address: H160([0x22; 20]),
                    factory: Some(H160([0x55; 20])),
                    tokens: vec![
                        StableToken {
                            address: H160([0x33; 20]),
                            decimals: 3,
                        },
                        StableToken {
                            address: H160([0x44; 20]),
                            decimals: 4,
                        },
                    ],
                }],
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
    fn convert_pool_to_registered_pool() {
        // Note that this test also demonstrates unreachable code is indeed unreachable
        use pools_query::*;
        let common = CommonPoolData {
            pool_id: H256([2; 32]),
            pool_address: H160([1; 20]),
            tokens: vec![H160([2; 20]), H160([3; 20])],
            scaling_exponents: vec![17, 16],
            block_created: 42,
        };

        let weighted_pool_data = PoolData {
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: None,
            tokens: vec![
                WeightedToken {
                    address: H160([2; 20]),
                    decimals: 1,
                    weight: "1.337".parse().unwrap(),
                },
                WeightedToken {
                    address: H160([3; 20]),
                    decimals: 2,
                    weight: "4.2".parse().unwrap(),
                },
            ],
        };

        assert_eq!(
            weighted_pool_data.into_weighted_pool(42).unwrap(),
            RegisteredWeightedPool {
                common: common.clone(),
                normalized_weights: vec![
                    Bfp::from_wei(1_337_000_000_000_000_000u128.into()),
                    Bfp::from_wei(4_200_000_000_000_000_000u128.into()),
                ],
            }
        );

        let stable_pool_data = PoolData {
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: None,
            tokens: vec![
                StableToken {
                    address: H160([2; 20]),
                    decimals: 1,
                },
                StableToken {
                    address: H160([3; 20]),
                    decimals: 2,
                },
            ],
        };

        assert_eq!(
            stable_pool_data.into_stable_pool(42).unwrap(),
            RegisteredStablePool { common }
        );
    }

    #[test]
    fn pool_conversion_invalid_decimals() {
        use pools_query::*;

        let pool = PoolData {
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: None,
            tokens: vec![WeightedToken {
                address: H160([2; 20]),
                decimals: 19,
                weight: "1.337".parse().unwrap(),
            }],
        };
        assert_eq!(
            pool.into_weighted_pool(2).unwrap_err().to_string(),
            "unsupported token with more than 18 decimals"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_subgraph_query() {
        let client = BalancerSubgraphClient::for_chain(1, Client::new()).unwrap();
        let pools = client.get_registered_pools().await.unwrap();
        println!(
            "Retrieved {} total weighted pools at block {}",
            pools
                .weighted_pools_by_factory
                .iter()
                .map(|(factory, pool)| {
                    println!("Retrieved {} pools for factory at {}", pool.len(), factory);
                    pool.len()
                })
                .sum::<usize>(),
            pools.fetched_block_number,
        );
        println!(
            "Retrieved {} total stable pools at block {}",
            pools
                .stable_pools_by_factory
                .iter()
                .map(|(factory, pool)| {
                    println!("Retrieved {} pools for factory at {}", pool.len(), factory);
                    pool.len()
                })
                .sum::<usize>(),
            pools.fetched_block_number,
        );
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
    */
}
