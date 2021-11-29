//! Module containing The Graph API client used for retrieving Balancer weighted
//! pools from the Balancer V2 subgraph.
//!
//! The pools retrieved from this client are used to prime the graph event store
//! to reduce start-up time. We do not use this in general for retrieving pools
//! as to:
//! - not rely on external services
//! - ensure that we are using the latest up-to-date pool data by using events
//!   from the node

use self::pools_query::{PoolData, PoolType};
use crate::{
    event_handling::MAX_REORG_BLOCK_COUNT,
    sources::balancer_v2::pool_storage::{
        CommonPoolData, RegisteredStablePool, RegisteredWeightedPool,
    },
    subgraph::SubgraphClient,
};
use anyhow::{anyhow, bail, Result};
use ethcontract::{H160, H256};
use reqwest::Client;
use serde_json::json;
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

        RegisteredPools::from_pool_data(block_number, pools)
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
    pub weighted_pools_by_factory: HashMap<H160, Vec<RegisteredWeightedPool>>,
    pub stable_pools_by_factory: HashMap<H160, Vec<RegisteredStablePool>>,
}

impl RegisteredPools {
    fn from_pool_data(fetched_block_number: u64, pools: Vec<PoolData>) -> Result<Self> {
        let mut registered_pools = Self {
            fetched_block_number,
            weighted_pools_by_factory: Default::default(),
            stable_pools_by_factory: Default::default(),
        };

        for pool in pools {
            let common = CommonPoolData {
                pool_id: pool.id,
                pool_address: pool.address,
                tokens: pool.tokens.iter().map(|token| token.address).collect(),
                scaling_exponents: pool
                    .tokens
                    .iter()
                    .map(|token| scaling_exponent_from_decimals(token.decimals))
                    .collect::<Result<_>>()?,
                block_created: fetched_block_number,
            };
            match pool.pool_type {
                PoolType::Weighted => registered_pools
                    .weighted_pools_by_factory
                    .entry(pool.factory)
                    .or_default()
                    .push(RegisteredWeightedPool {
                        common,
                        normalized_weights: pool
                            .tokens
                            .iter()
                            .map(|token| {
                                token.weight.ok_or_else(|| {
                                    anyhow!("missing weights for pool {:?}", pool.id)
                                })
                            })
                            .collect::<Result<_>>()?,
                    }),
                PoolType::Stable => registered_pools
                    .stable_pools_by_factory
                    .entry(pool.factory)
                    .or_default()
                    .push(RegisteredStablePool { common }),
            }
        }

        Ok(registered_pools)
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
    use crate::sources::balancer_v2::swap::fixed_point::Bfp;
    use ethcontract::{H160, H256};
    use serde::Deserialize;
    use serde_with::{serde_as, DisplayFromStr};

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
    #[derive(Debug, Deserialize, PartialEq)]
    pub enum PoolType {
        Stable,
        Weighted,
    }

    #[serde_as]
    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Token {
        pub address: H160,
        pub decimals: u8,
        #[serde_as(as = "Option<DisplayFromStr>")]
        #[serde(default)]
        pub weight: Option<Bfp>,
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
    use maplit::hashmap;

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
    fn convert_pools_to_registered_pools() {
        // Note that this test also demonstrates unreachable code is indeed unreachable
        use pools_query::*;

        let pools = vec![
            PoolData {
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
            },
            PoolData {
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
            },
        ];

        assert_eq!(
            RegisteredPools::from_pool_data(42, pools).unwrap(),
            RegisteredPools {
                fetched_block_number: 42,
                weighted_pools_by_factory: hashmap! {
                    H160([0xfa; 20]) => vec![RegisteredWeightedPool {
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
                    }],
                },
                stable_pools_by_factory: hashmap! {
                    H160([0xfb; 20]) => vec![RegisteredStablePool {
                        common: CommonPoolData {
                            pool_id: H256([4; 32]),
                            pool_address: H160([3; 20]),
                            tokens: vec![H160([0x33; 20]), H160([0x44; 20])],
                            scaling_exponents: vec![15, 0],
                            block_created: 42,
                        },
                    }],
                },
            }
        );
    }

    #[test]
    fn pool_conversion_invalid_decimals() {
        use pools_query::*;

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
        assert!(RegisteredPools::from_pool_data(0, vec![pool]).is_err())
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
    }
}
