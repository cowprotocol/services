//! Module containing The Graph API client used for retrieving Balancer weighted
//! pools from the Balancer V2 subgraph.
//!
//! The pools retrieved from this client are used to prime the graph event store
//! to reduce start-up time. We do not use this in general for retrieving pools
//! as to:
//! - not rely on external services
//! - ensure that we are using the latest up-to-date pool data by using events
//!   from the node

use {
    super::swap::fixed_point::Bfp,
    crate::{event_handling::MAX_REORG_BLOCK_COUNT, subgraph::SubgraphClient},
    alloy::primitives::{Address, B256},
    anyhow::Result,
    reqwest::{Client, Url},
    serde::Deserialize,
    serde_json::json,
    serde_with::{DisplayFromStr, serde_as},
    std::collections::HashMap,
};

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
    /// Creates a new Balancer subgraph client with full subgraph URL.
    pub fn from_subgraph_url(subgraph_url: &Url, client: Client) -> Result<Self> {
        Ok(Self(SubgraphClient::try_new(
            subgraph_url.clone(),
            client,
            usize::MAX,
        )?))
    }

    /// Retrieves the list of registered pools from the subgraph.
    pub async fn get_registered_pools(&self) -> Result<RegisteredPools> {
        use self::pools_query::*;

        let block_number = self.get_safe_block().await?;

        let mut pools = Vec::new();
        let mut last_id = B256::default();

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
#[derive(Debug, Default, Eq, PartialEq)]
pub struct RegisteredPools {
    /// The block number that the data was fetched, and for which the registered
    /// weighted pools can be considered up to date.
    pub fetched_block_number: u64,
    /// The registered Pools
    pub pools: Vec<PoolData>,
}

impl RegisteredPools {
    /// Creates an empty collection of registered pools for the specified block
    /// number.
    pub fn empty(fetched_block_number: u64) -> Self {
        Self {
            fetched_block_number,
            ..Default::default()
        }
    }

    /// Groups registered pools by factory addresses.
    pub fn group_by_factory(self) -> HashMap<Address, RegisteredPools> {
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
    pub id: B256,
    pub address: Address,
    pub factory: Address,
    pub swap_enabled: bool,
    pub tokens: Vec<Token>,
}

/// Supported pool kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
pub enum PoolType {
    Stable,
    Weighted,
    LiquidityBootstrapping,
    ComposableStable,
}

/// Token data for pools.
#[serde_as]
#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Token {
    pub address: Address,
    pub decimals: u8,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub weight: Option<Bfp>,
}

mod pools_query {
    use {super::PoolData, serde::Deserialize};

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
                        "ComposableStable",
                    ]
                    totalLiquidity_gt: "1" # 1$ value of tokens
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
    use {
        super::*,
        crate::sources::balancer_v2::swap::fixed_point::Bfp,
        alloy::primitives::U256,
        maplit::hashmap,
    };

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
                    {
                        "poolType": "ComposableStable",
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
                ],
            }))
            .unwrap(),
            Data {
                pools: vec![
                    PoolData {
                        pool_type: PoolType::Weighted,
                        id: B256::repeat_byte(0x11),
                        address: Address::repeat_byte(0x22),
                        factory: Address::repeat_byte(0x55),
                        swap_enabled: true,
                        tokens: vec![
                            Token {
                                address: Address::repeat_byte(0x33),
                                decimals: 3,
                                weight: Some(Bfp::from_wei(U256::from(
                                    500_000_000_000_000_000_u128
                                ))),
                            },
                            Token {
                                address: Address::repeat_byte(0x44),
                                decimals: 4,
                                weight: Some(Bfp::from_wei(U256::from(
                                    500_000_000_000_000_000_u128
                                ))),
                            },
                        ],
                    },
                    PoolData {
                        pool_type: PoolType::Stable,
                        id: B256::repeat_byte(0x11),
                        address: Address::repeat_byte(0x22),
                        factory: Address::repeat_byte(0x55),
                        swap_enabled: true,
                        tokens: vec![
                            Token {
                                address: Address::repeat_byte(0x33),
                                decimals: 3,
                                weight: None,
                            },
                            Token {
                                address: Address::repeat_byte(0x44),
                                decimals: 4,
                                weight: None,
                            },
                        ],
                    },
                    PoolData {
                        pool_type: PoolType::LiquidityBootstrapping,
                        id: B256::repeat_byte(0x11),
                        address: Address::repeat_byte(0x22),
                        factory: Address::repeat_byte(0x55),
                        swap_enabled: true,
                        tokens: vec![
                            Token {
                                address: Address::repeat_byte(0x33),
                                decimals: 3,
                                weight: Some(Bfp::from_wei(U256::from(
                                    500_000_000_000_000_000_u128
                                ))),
                            },
                            Token {
                                address: Address::repeat_byte(0x44),
                                decimals: 4,
                                weight: Some(Bfp::from_wei(U256::from(
                                    500_000_000_000_000_000_u128
                                ))),
                            },
                        ],
                    },
                    PoolData {
                        pool_type: PoolType::ComposableStable,
                        id: B256::repeat_byte(0x11),
                        address: Address::repeat_byte(0x22),
                        factory: Address::repeat_byte(0x55),
                        swap_enabled: true,
                        tokens: vec![
                            Token {
                                address: Address::repeat_byte(0x33),
                                decimals: 3,
                                weight: None,
                            },
                            Token {
                                address: Address::repeat_byte(0x44),
                                decimals: 4,
                                weight: None,
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
        let pool = |factory: Address, id: u8| PoolData {
            id: B256::repeat_byte(id),
            factory,
            pool_type: PoolType::Weighted,
            address: Default::default(),
            swap_enabled: true,
            tokens: Default::default(),
        };

        let registered_pools = RegisteredPools {
            pools: vec![
                pool(Address::repeat_byte(1), 1),
                pool(Address::repeat_byte(1), 2),
                pool(Address::repeat_byte(2), 3),
            ],
            fetched_block_number: 42,
        };

        assert_eq!(
            registered_pools.group_by_factory(),
            hashmap! {
                Address::repeat_byte(1) => RegisteredPools {
                    pools: vec![
                        pool(Address::repeat_byte(1), 1),
                        pool(Address::repeat_byte(1), 2),
                    ],
                    fetched_block_number: 42,
                },
                Address::repeat_byte(2) => RegisteredPools {
                    pools: vec![
                        pool(Address::repeat_byte(2), 3),
                    ],
                    fetched_block_number: 42,
                },
            }
        )
    }
}
