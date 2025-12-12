//! Module containing The Graph API client used for retrieving Uniswap V3
//! data from the Uniswap V3 subgraph.

use {
    crate::{
        event_handling::MAX_REORG_BLOCK_COUNT,
        subgraph::{ContainsId, SubgraphClient},
    },
    alloy::primitives::{Address, U256},
    anyhow::Result,
    num::BigInt,
    number::serialization::HexOrDecimalU256,
    reqwest::{Client, Url},
    serde::{Deserialize, Serialize},
    serde_json::{Map, Value, json},
    serde_with::{DisplayFromStr, serde_as},
    std::collections::HashMap,
};

// Some subgraphs don't have a the ticks_ filter. Use this query to check for
// its presence.
const CHECK_LIQUIDITY_NET_FILTER: &str = r#"
query CheckLiquidityNetField($block: Int) {
  pools(
    first: 1
    where: { ticks_: { liquidityNet_not: "0" } }
  ) {
    id
  }
}
"#;

const TICKS_BY_POOL_IDS_QUERY: &str = r#"
    query Ticks($block: Int, $pool_ids: [ID], $pageSize: Int, $lastId: ID) {
        ticks(
            block: { number: $block }
            first: $pageSize
            where: {
                id_gt: $lastId
                liquidityNet_not: "0"
                pool_: { id_in: $pool_ids }
            }
        ) {
            id
            tickIdx
            liquidityNet
            poolAddress
        }
    }
"#;

/// A client to the Uniswap V3 subgraph.
///
/// This client is not implemented to allow general GraphQL queries, but instead
/// implements high-level methods that perform GraphQL queries under the hood.
pub struct UniV3SubgraphClient {
    client: SubgraphClient,
    /// Some subgraphs do not support the liquidityNet filter on ticks.
    /// This flag indicates whether to use it or not in queries.
    use_liquidity_net_filter: bool,
}

impl UniV3SubgraphClient {
    /// Creates a new Uniswap V3 subgraph client from the specified URL.
    pub async fn from_subgraph_url(
        subgraph_url: &Url,
        client: Client,
        max_pools_per_tick_query: usize,
    ) -> Result<Self> {
        let subgraph_client =
            SubgraphClient::try_new(subgraph_url.clone(), client, max_pools_per_tick_query)?;

        Ok(Self {
            client: subgraph_client,
            use_liquidity_net_filter: true,
        }
        .set_liquidity_net_filter()
        .await)
    }

    // Try a simple query to verify that the liquidityNet filter is supported
    async fn set_liquidity_net_filter(mut self) -> Self {
        let result: Result<serde_json::Value> = self
            .client
            .query_without_retry::<serde_json::Value>(CHECK_LIQUIDITY_NET_FILTER, &None)
            .await;

        if let Err(err) = &result
            && err.to_string().contains("liquidityNet_not")
        {
            // If the query fails, it likely means the subgraph does not support the
            // liquidityNet filter.
            self.use_liquidity_net_filter = false;
        }

        self
    }

    async fn get_pools(
        &self,
        query: String,
        variables: Map<String, Value>,
    ) -> Result<Vec<PoolData>> {
        Ok(self
            .client
            .paginated_query(&query, variables)
            .await?
            .into_iter()
            .filter(|pool: &PoolData| pool.liquidity > U256::ZERO)
            .collect())
    }

    /// Retrieves the pool data for all existing pools from the subgraph.
    pub async fn get_registered_pools(&self) -> Result<RegisteredPools> {
        let block_number = self.get_safe_block().await?;
        let variables = json_map! {
            "block" => block_number,
        };
        let query = Self::all_pools_query(self.use_liquidity_net_filter);
        let pools = self.get_pools(query, variables).await?;
        Ok(RegisteredPools {
            fetched_block_number: block_number,
            pools,
        })
    }

    async fn get_pools_by_pool_ids(
        &self,
        pool_ids: &[Address],
        block_number: u64,
    ) -> Result<Vec<PoolData>> {
        let variables = json_map! {
            "block" => block_number,
            "pool_ids" => json!(pool_ids)
        };
        let query = Self::pools_by_ids_query(self.use_liquidity_net_filter);
        let pools = self.get_pools(query, variables).await?;
        Ok(pools)
    }

    /// Retrieves the ticks data for pools with given pool ids
    async fn get_ticks_by_pools_ids(
        &self,
        pool_ids: &[Address],
        block_number: u64,
    ) -> Result<Vec<TickData>> {
        let mut all = Vec::new();

        // Default chunk size is usize::MAX - all pool ids in one `where`. We want to
        // run requests sequentially to avoid overwhelming the node.
        for chunk in pool_ids.chunks(self.client.max_pools_per_tick_query()) {
            let variables = json_map! {
                "block" => block_number,
                "pool_ids" => json!(chunk)
            };
            let mut batch = self
                .client
                .paginated_query(TICKS_BY_POOL_IDS_QUERY, variables)
                .await?;
            all.append(&mut batch);
        }

        Ok(all)
    }

    /// Retrieves the pool data and ticks data for pools with given pool ids
    pub async fn get_pools_with_ticks_by_ids(
        &self,
        ids: &[Address],
        block_number: u64,
    ) -> Result<Vec<PoolData>> {
        let (pools, ticks) = futures::try_join!(
            self.get_pools_by_pool_ids(ids, block_number),
            self.get_ticks_by_pools_ids(ids, block_number)
        )?;

        // group ticks by pool ids
        let mut ticks_mapped = HashMap::new();
        for tick in ticks {
            ticks_mapped
                .entry(tick.pool_address)
                .or_insert_with(Vec::new)
                .push(tick);
        }

        Ok(pools
            .into_iter()
            .filter_map(|mut pool| {
                ticks_mapped.get(&pool.id).map(|ticks| {
                    pool.ticks = Some(ticks.clone());
                    pool
                })
            })
            .collect())
    }

    /// Retrieves a recent block number for which it is safe to assume no
    /// reorgs will happen.
    pub async fn get_safe_block(&self) -> Result<u64> {
        // Ideally we would want to use block hash here so that we can check
        // that there indeed is no reorg. However, it does not seem possible to
        // retrieve historic block hashes just from the subgraph (it always
        // returns `null`).
        Ok(self
            .client
            .query::<block_number_query::Data>(block_number_query::QUERY, None)
            .await?
            .meta
            .block
            .number
            .saturating_sub(MAX_REORG_BLOCK_COUNT))
    }

    fn all_pools_query(include_ticks_filter: bool) -> String {
        let tick_filter = if include_ticks_filter {
            r#"ticks_: { liquidityNet_not: "0" }"#
        } else {
            ""
        };

        format!(
            r#"
            query Pools($block: Int, $pageSize: Int, $lastId: ID) {{
                pools(
                    block: {{ number: $block }}
                    first: $pageSize
                    where: {{
                        id_gt: $lastId
                        tick_not: null
                        {tick_filter}
                    }}
                ) {{
                    id
                    token0 {{ symbol id decimals }}
                    token1 {{ symbol id decimals }}
                    feeTier
                    liquidity
                    sqrtPrice
                    tick
                }}
            }}
            "#
        )
    }

    fn pools_by_ids_query(include_ticks_filter: bool) -> String {
        let tick_filter = if include_ticks_filter {
            r#"ticks_: { liquidityNet_not: "0" }"#
        } else {
            "liquidity_not: 0"
        };

        format!(
            r#"
            query Pools($block: Int, $pool_ids: [ID], $pageSize: Int, $lastId: ID) {{
                pools(
                    block: {{ number: $block }}
                    first: $pageSize
                    where: {{
                        id_in: $pool_ids
                        id_gt: $lastId
                        tick_not: null
                        {tick_filter}
                    }}
                ) {{
                    id
                    token0 {{ symbol id decimals }}
                    token1 {{ symbol id decimals }}
                    feeTier
                    liquidity
                    sqrtPrice
                    tick
                }}
            }}
            "#
        )
    }
}

/// Result of the registered stable pool query.
#[derive(Debug, Default, PartialEq)]
pub struct RegisteredPools {
    /// The block number that the data was fetched
    pub fetched_block_number: u64,
    /// The registered Pools
    pub pools: Vec<PoolData>,
}

/// Pool data from the Uniswap V3 subgraph.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolData {
    pub id: Address,
    pub token0: Token,
    pub token1: Token,
    #[serde_as(as = "HexOrDecimalU256")]
    pub fee_tier: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub liquidity: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sqrt_price: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub tick: BigInt,
    pub ticks: Option<Vec<TickData>>,
}

impl ContainsId for PoolData {
    fn get_id(&self) -> String {
        format!("{:#x}", self.id)
    }
}

/// Tick data from the Uniswap V3 subgraph.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TickData {
    pub id: String,
    #[serde_as(as = "DisplayFromStr")]
    pub tick_idx: BigInt,
    #[serde_as(as = "DisplayFromStr")]
    pub liquidity_net: BigInt,
    pub pool_address: Address,
}

impl ContainsId for TickData {
    fn get_id(&self) -> String {
        self.id.clone()
    }
}

#[serde_as]
#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: Address,
    #[serde_as(as = "DisplayFromStr")]
    pub decimals: u8,
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
    use {super::*, crate::subgraph::Data, alloy::primitives::address, serde_json::json};

    #[test]
    fn decode_pools_data() {
        assert_eq!(
            serde_json::from_value::<Data<PoolData>>(json!({
                "pools": [
                    {
                      "id": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28",
                      "token0": {
                        "decimals": "18",
                        "id": "0xbef81556ef066ec840a540595c8d12f516b6378f",
                        "symbol": "BCZ"
                      },
                      "token1": {
                        "decimals": "18",
                        "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "symbol": "WETH"
                      },
                      "feeTier": "10000",
                      "liquidity": "303015134493562686441",
                      "tick": "-92110",
                      "sqrtPrice": "792216481398733702759960397"
                    },
                    {
                      "id": "0x0002e63328169d7feea121f1e32e4f620abf0352",
                      "token0": {
                        "decimals": "18",
                        "id": "0x0d438f3b5175bebc262bf23753c1e53d03432bde",
                        "symbol": "wNXM"
                      },
                      "token1": {
                        "decimals": "9",
                        "id": "0x903bef1736cddf2a537176cf3c64579c3867a881",
                        "symbol": "ICHI"
                      },
                      "feeTier": "3000",
                      "liquidity": "3125586395511534995",
                      "tick": "-189822",
                      "sqrtPrice": "5986323062404391218190509"
                    }
                ],
            }))
            .unwrap(),
            Data {
                inner: vec![
                    PoolData {
                        id: address!("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28"),
                        token0: Token {
                            id: address!("0xbef81556ef066ec840a540595c8d12f516b6378f"),
                            decimals: 18,
                        },
                        token1: Token {
                            id: address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                            decimals: 18,
                        },
                        fee_tier: U256::from(10000),
                        liquidity: U256::from(303015134493562686441_u128),
                        sqrt_price: U256::from(792216481398733702759960397_u128),
                        tick: BigInt::from(-92110),
                        ticks: None,
                    },
                    PoolData {
                        id: address!("0x0002e63328169d7feea121f1e32e4f620abf0352"),
                        token0: Token {
                            id: address!("0x0d438f3b5175bebc262bf23753c1e53d03432bde"),
                            decimals: 18,
                        },
                        token1: Token {
                            id: address!("0x903bef1736cddf2a537176cf3c64579c3867a881"),
                            decimals: 9,
                        },
                        fee_tier: U256::from(3000),
                        liquidity: U256::from(3125586395511534995_u128),
                        sqrt_price: U256::from(5986323062404391218190509_u128),
                        tick: BigInt::from(-189822),
                        ticks: None,
                    },
                ],
            }
        );
    }

    #[test]
    fn decode_ticks_data() {
        assert_eq!(
            serde_json::from_value::<Data<TickData>>(json!({
                "ticks": [
                    {
                      "id": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#0",
                      "tickIdx": "0",
                      "liquidityNet": "-303015134493562686441",
                      "poolAddress": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28"
                    },
                    {
                      "id": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#-92200",
                      "tickIdx": "-92200",
                      "liquidityNet": "303015134493562686441",
                      "poolAddress": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28"
                    }
                ],
            }))
            .unwrap(),
            Data {
                inner: vec![
                    TickData {
                        id: "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#0".to_string(),
                        tick_idx: BigInt::from(0),
                        liquidity_net: BigInt::from(-303015134493562686441i128),
                        pool_address: address!("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28")
                    },
                    TickData {
                        id: "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28#-92200".to_string(),
                        tick_idx: BigInt::from(-92200),
                        liquidity_net: BigInt::from(303015134493562686441i128),
                        pool_address: address!("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28")
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
}
