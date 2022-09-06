//! Module containing The Graph API client used for retrieving Balancer weighted
//! pools from the Balancer V2 subgraph.
//!
//! The pools retrieved from this client are used to prime the graph event store
//! to reduce start-up time. We do not use this in general for retrieving pools
//! as to:
//! - not rely on external services
//! - ensure that we are using the latest up-to-date pool data by using events
//!   from the node

use super::swap::fixed_point::Bfp;
use crate::{
    event_handling::{BlockNumberHash, MAX_REORG_BLOCK_COUNT},
    subgraph::SubgraphClient,
    Web3,
};
use anyhow::{bail, Context, Result};
use ethcontract::{H160, H256};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use serde_with::{serde_as, DisplayFromStr};
use std::collections::HashMap;
use web3::types::U64;

/// The page size when querying pools.
#[cfg(not(test))]
const QUERY_PAGE_SIZE: usize = 1000;
#[cfg(test)]
const QUERY_PAGE_SIZE: usize = 10;

/// A client to the Balancer V2 subgraph.
///
/// This client is not implemented to allow general GraphQL queries, but instead
/// implements high-level methods that perform GraphQL queries under the hood.
pub struct BalancerSubgraphClient {
    pub graph: SubgraphClient,
    pub web3: Web3,
}

impl BalancerSubgraphClient {
    /// Creates a new Balancer subgraph client for the specified chain ID.
    pub fn for_chain(chain_id: u64, client: Client, web3: Web3) -> Result<Self> {
        let subgraph_name = match chain_id {
            1 => "balancer-v2",
            4 => "balancer-rinkeby-v2",
            5 => "balancer-goerli-v2",
            _ => bail!("unsupported chain {}", chain_id),
        };
        Ok(BalancerSubgraphClient {
            graph: SubgraphClient::new("balancer-labs", subgraph_name, client)?,
            web3,
        })
    }

    /// Retrieves the list of registered pools from the subgraph.
    pub async fn get_registered_pools(&self) -> Result<RegisteredPools> {
        use self::pools_query::*;

        let block_number = self.get_safe_block().await?;
        let block_number_hash = self
            .web3
            .eth()
            .block(U64::from(block_number).into())
            .await?
            .context("missing block")?
            .hash
            .context("no hash in block - pending block")?;

        let mut pools = Vec::new();
        let mut last_id = H256::default();

        // We do paging by last ID instead of using `skip`. This is the
        // suggested approach to paging best performance:
        // <https://thegraph.com/docs/graphql-api#pagination>
        loop {
            let page = self
                .graph
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
            fetched_block_number: (block_number, block_number_hash),
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
            .graph
            .query::<block_number_query::Data>(block_number_query::QUERY, None)
            .await?
            .meta
            .block
            .number
            .saturating_sub(MAX_REORG_BLOCK_COUNT))
    }
}

/// Result of the registered stable pool query.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct RegisteredPools {
    /// The block number that the data was fetched, and for which the registered
    /// weighted pools can be considered up to date.
    pub fetched_block_number: BlockNumberHash,
    /// The registered Pools
    pub pools: Vec<PoolData>,
}

impl RegisteredPools {
    /// Creates an empty collection of registered pools for the specified block
    /// number.
    pub fn empty(fetched_block_number: BlockNumberHash) -> Self {
        Self {
            fetched_block_number,
            ..Default::default()
        }
    }

    /// Groups registered pools by factory addresses.
    pub fn group_by_factory(self) -> HashMap<H160, RegisteredPools> {
        let fetched_block_number = self.fetched_block_number;
        self.pools
            .into_iter()
            .fold(HashMap::new(), |mut grouped, pool| {
                grouped
                    .entry(pool.factory)
                    .or_insert(RegisteredPools {
                        fetched_block_number,
                        ..Default::default()
                    })
                    .pools
                    .push(pool);
                grouped
            })
    }
}

/// Pool data from the Balancer V2 subgraph.
#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolData {
    pub pool_type: PoolType,
    pub id: H256,
    pub address: H160,
    pub factory: H160,
    pub swap_enabled: bool,
    pub tokens: Vec<Token>,
}

/// Supported pool kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
pub enum PoolType {
    Stable,
    Weighted,
    LiquidityBootstrapping,
}

/// Token data for pools.
#[serde_as]
#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Token {
    pub address: H160,
    pub decimals: u8,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
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
                    poolType_in: [
                        "Stable",
                        "Weighted",
                        "LiquidityBootstrapping",
                    ]
                }
            ) {
                poolType
                id
                address
                factory
                swapEnabled
                tokens {
                    address
                    decimals
                    weight
                }
            }
        }
    "#;

    #[derive(Debug, Deserialize, Eq, PartialEq)]
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

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    pub struct Data {
        #[serde(rename = "_meta")]
        pub meta: Meta,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    pub struct Meta {
        pub block: Block,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    pub struct Block {
        pub number: u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sources::balancer_v2::swap::fixed_point::Bfp, transport::create_env_test_transport,
    };
    use ethcontract::{H160, H256};
    use maplit::hashmap;
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
                        "swapEnabled": true,
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
                        "swapEnabled": true,
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
                    {
                        "poolType": "LiquidityBootstrapping",
                        "address": "0x2222222222222222222222222222222222222222",
                        "id": "0x1111111111111111111111111111111111111111111111111111111111111111",
                        "factory": "0x5555555555555555555555555555555555555555",
                        "swapEnabled": true,
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
                pools: vec![
                    PoolData {
                        pool_type: PoolType::Weighted,
                        id: H256([0x11; 32]),
                        address: H160([0x22; 20]),
                        factory: H160([0x55; 20]),
                        swap_enabled: true,
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
                        swap_enabled: true,
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
                    },
                    PoolData {
                        pool_type: PoolType::LiquidityBootstrapping,
                        id: H256([0x11; 32]),
                        address: H160([0x22; 20]),
                        factory: H160([0x55; 20]),
                        swap_enabled: true,
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
    fn groups_pools_by_factory() {
        let pool = |factory: H160, id: u8| PoolData {
            id: H256([id; 32]),
            factory,
            pool_type: PoolType::Weighted,
            address: Default::default(),
            swap_enabled: true,
            tokens: Default::default(),
        };

        let registered_pools = RegisteredPools {
            pools: vec![
                pool(H160([1; 20]), 1),
                pool(H160([1; 20]), 2),
                pool(H160([2; 20]), 3),
            ],
            fetched_block_number: (42, H256::from_low_u64_be(42)),
        };

        assert_eq!(
            registered_pools.group_by_factory(),
            hashmap! {
                H160([1; 20]) => RegisteredPools {
                    pools: vec![
                        pool(H160([1; 20]), 1),
                        pool(H160([1; 20]), 2),
                    ],
                    fetched_block_number: (42, H256::from_low_u64_be(42)),
                },
                H160([2; 20]) => RegisteredPools {
                    pools: vec![
                        pool(H160([2; 20]), 3),
                    ],
                    fetched_block_number: (42, H256::from_low_u64_be(42)),
                },
            }
        )
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_subgraph_query() {
        for (network_name, chain_id) in [("Mainnet", 1), ("Rinkeby", 4)] {
            println!("### {}", network_name);

            let transport = create_env_test_transport();
            let web3 = Web3::new(transport);
            let client = BalancerSubgraphClient::for_chain(chain_id, Client::new(), web3).unwrap();
            let result = client.get_registered_pools().await.unwrap();
            println!(
                "Retrieved {} total pools at block {:?}",
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
